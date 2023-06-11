use std::collections::HashSet;

use crate::prelude::*;
use kamilata::{behaviour::KamilataEvent, db::{TooManySeeders, TooManyLeechers}};
use libp2p::{swarm::{Swarm, SwarmBuilder, SwarmEvent, NetworkBehaviour}, identity::Keypair, PeerId, tcp, Transport, core::upgrade, mplex::MplexConfig, noise, Multiaddr};
use libp2p_identify::{Behaviour as IdentifyBehaviour, Event as IdentifyEvent, Config as IdentifyConfig};
use tokio::sync::{mpsc::*, oneshot::{Sender as OneshotSender, channel as oneshot_channel}};
use futures::{StreamExt, future};

const FILTER_SIZE: usize = 125000;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "Event")]
struct AdmarusBehaviour {
    kamilata: KamilataBehaviour<FILTER_SIZE, DocumentIndex<FILTER_SIZE>>,
    identify: IdentifyBehaviour,
}

#[derive(Debug)]
enum Event {
    Identify(Box<IdentifyEvent>),
    Kamilata(KamilataEvent),
}

impl From<IdentifyEvent> for Event {
    fn from(event: IdentifyEvent) -> Self {
        Self::Identify(Box::new(event))
    }
}
  
impl From<KamilataEvent> for Event {
    fn from(event: KamilataEvent) -> Self {
        Self::Kamilata(event)
    }
}

struct ConnectedPeerInfo {
    selected: bool,
    seeding: bool,
    leeching: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ConnectedPeerRole {
    Seeder,
    Leecher,
    Transient
}

impl ConnectedPeerInfo {
    pub fn role(&self) -> ConnectedPeerRole {
        if self.selected {
            ConnectedPeerRole::Seeder
        } else if self.leeching {
            ConnectedPeerRole::Leecher
        } else {
            ConnectedPeerRole::Transient
        }
    }
}

struct KamilataState {
    config: Arc<Args>,
    known_peers: RwLock<HashMap<PeerId, PeerInfo>>,

    connected_peers: RwLock<HashMap<PeerId, ConnectedPeerInfo>>,
}

impl KamilataState {
    pub fn new(config: Arc<Args>) -> KamilataState {
        KamilataState {
            config,
            known_peers: RwLock::new(HashMap::new()),
            connected_peers: RwLock::new(HashMap::new()),
        }
    }
}

impl KamilataState {
    pub async fn role_counts(&self) -> (usize, usize, usize) {
        let mut seeders = 0;
        let mut leechers = 0;
        let mut transient_peers = 0;
        self.connected_peers.read().await.values().for_each(|i| match i.role() {
            ConnectedPeerRole::Seeder => seeders += 1,
            ConnectedPeerRole::Leecher => leechers += 1,
            ConnectedPeerRole::Transient => transient_peers += 1,
        });
        (seeders, leechers, transient_peers)
    }

    pub async fn seeder_available(&self) -> bool {
        let seeder_count = self.role_counts().await.0;
        seeder_count < self.config.seeders
    }

    pub async fn leecher_available(&self) -> bool {
        let leecher_count = self.role_counts().await.1;
        leecher_count < self.config.leechers
    }

    pub async fn role(&self, peer_id: &PeerId) -> Option<ConnectedPeerRole> {
        self.connected_peers.read().await.get(peer_id).map(|i| i.role())
    }

    async fn on_seeder_added(&self, peer_id: PeerId) {
        let mut connected_peers = self.connected_peers.write().await;
        connected_peers.entry(peer_id).and_modify(|i| i.seeding = true);
    }

    async fn on_leecher_added(&self, peer_id: PeerId) -> Result<(), TooManyLeechers> {
        let mut connected_peers = self.connected_peers.write().await;
        connected_peers.entry(peer_id).and_modify(|i| i.leeching = true);
        let leecher_count = connected_peers.values().filter(|i| i.role() == ConnectedPeerRole::Leecher).count();
        if leecher_count > self.config.leechers {
            return Err(TooManyLeechers{})
        }
        Ok(())
    }

    async fn on_seeder_removed(&self, peer_id: &PeerId) {
        let mut connected_peers = self.connected_peers.write().await;
        connected_peers.entry(*peer_id).and_modify(|i| i.seeding = false);
    }

    async fn on_leecher_removed(&self, peer_id: &PeerId) {
        let mut connected_peers = self.connected_peers.write().await;
        connected_peers.entry(*peer_id).and_modify(|i| i.leeching = false);
    }
}


/// # Peer swarm managment 
/// 
/// This implementation attributes slots to different kind of peers.
/// The policies are strongly enforced, and the swarm isn't reluctant to disconnect peers.
/// 
/// ## Seeders
/// 
/// Those are select peers we chose to leech from.
/// We chose those whom we trust the most.
/// We try to reach SEEDER_TARGET, and we never go above.
/// These peers have guaranteed slots as leechers too.
/// = SEEDER_TARGET
/// 
/// ## Leechers
/// 
/// Those are peers who selected us to leech from.
/// We leech back from all leechers, though they don't count as seeders.
/// Leechers have the right to refuse to seed us.
/// When new peers apply for a leecher slot and they are all taken, we disconnect the peer with the lowest score.
/// In order to prevent a malicious actor from replacing all legitimate leechers, peers that cause a disconnection start with a reputation malus.
/// <= MAX_LEECHERS
/// 
/// ## Transient peers
/// 
/// Some peers connect for a few seconds, the time to send us queries. We do the same to them.
/// Those peers are theoretically unlimited, but there is a practical high limit at MAX_FAST_PACED_SLOTS.
/// The main limit is actually the time those peers are allowed to stay connected.
/// When that time is up, we disconnect them. We might be more tolerant when we have plenty of slots available.
/// <= MAX_FAST_PACED_SLOTS
pub struct KamilataNode {
    swarm: Swarm<AdmarusBehaviour>,
    state: Arc<KamilataState>,
}

struct PeerInfo {
    addrs: Vec<Multiaddr>,
}

impl KamilataNode {
    pub async fn init(config: Arc<Args>, index: DocumentIndex<FILTER_SIZE>) -> KamilataNode {
        let local_key = Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        let kam_state = Arc::new(KamilataState::new(Arc::clone(&config)));
        let kam_state2 = Arc::clone(&kam_state);
        let approve_leecher = move |peer_id: PeerId| -> Pin<Box<dyn Future<Output = bool> + Send>> {
            let kam_state3 = Arc::clone(&kam_state2);
            Box::pin(async move {
                kam_state3.leecher_available().await || kam_state3.role(&peer_id).await == Some(ConnectedPeerRole::Seeder)
            })
        };
        let kam_config = KamilataConfig {
            approve_leecher: Some(Box::new(approve_leecher)),
            ..KamilataConfig::default()
        };

        let kamilata = KamilataBehaviour::new_with_config_and_store(peer_id, kam_config, index);
        let identify = IdentifyBehaviour::new(
            IdentifyConfig::new(String::from("admarus/0.1.0"), local_key.public())
        );
        let behaviour = AdmarusBehaviour {
            kamilata,
            identify,
        };
        
        
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::new());

        let transport = tcp_transport
            .upgrade(upgrade::Version::V1Lazy)
            .authenticate(
                noise::Config::new(&local_key).expect("Signing libp2p-noise static DH keypair failed."),
            )
            .multiplex(MplexConfig::default())
            .boxed();
        
        let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build();
        swarm.listen_on(config.kam_addr.parse().unwrap()).unwrap();

        KamilataNode {
            swarm,
            state: kam_state,
        }
    }

    fn kam_mut(&mut self) -> &mut KamilataBehaviour<FILTER_SIZE, DocumentIndex<FILTER_SIZE>> {
        &mut self.swarm.behaviour_mut().kamilata
    }

    pub fn run(mut self) -> KamilataController {
        let (sender, mut receiver) = channel(1);
        let controller = 
        KamilataController {
            sender,
            state: Arc::clone(&self.state)
        };
        tokio::spawn(async move {
            loop {
                let recv = Box::pin(receiver.recv());
                let value = futures::future::select(recv, self.swarm.select_next_some()).await;
                match value {
                    future::Either::Left((Some(command), _)) => match command {
                        ClientCommand::Search { queries, config, sender } => {
                            let controller = self.kam_mut().search_with_config(queries, config).await;
                            let _ = sender.send(controller);
                        },
                        ClientCommand::Dial { addr } => {
                            self.swarm.dial(addr).unwrap();
                        },
                        ClientCommand::LeechFromAll => {
                            let peer_ids = self.swarm.connected_peers().cloned().collect::<Vec<_>>();
                            for peer_id in peer_ids {
                                trace!("Leeching from {:?}", peer_id);
                                self.kam_mut().leech_from(peer_id);
                            }
                        },
                    },
                    future::Either::Left((None, _)) => break,
                    future::Either::Right((event, _)) => match event {
                        SwarmEvent::Behaviour(Event::Identify(event)) => match *event {
                            IdentifyEvent::Received { peer_id, info } => {
                                let r = self.kam_mut().set_addresses(&peer_id, info.listen_addrs).await;
                                if let Err(e) = r {
                                    error!("Error while setting addresses for {peer_id:?}: {e:?}");
                                }
                            },
                            IdentifyEvent::Sent { peer_id } => trace!("Sent identify request to {peer_id:?}"),
                            IdentifyEvent::Pushed { peer_id } => trace!("Pushed identify info to {peer_id:?}"),
                            IdentifyEvent::Error { peer_id, error } => debug!("Identify error with {peer_id:?}: {error:?}"),
                        },
                        SwarmEvent::Behaviour(Event::Kamilata(event)) => match event {
                            KamilataEvent::LeecherAdded { peer_id, filter_count, interval_ms } => {
                                debug!("Leecher added: {peer_id:?} (filter_count: {filter_count:?}, interval_ms: {interval_ms:?})");
                                let r = self.state.on_leecher_added(peer_id).await;
                                if let Err(e) = r {
                                    error!("Error while adding leecher {peer_id:?}: {e:?}");
                                    self.kam_mut().stop_seeding(peer_id);
                                }
                            },
                            KamilataEvent::SeederAdded { peer_id } => {
                                debug!("Seeder added: {peer_id:?}");
                                self.state.on_seeder_added(peer_id).await;
                            },
                            KamilataEvent::LeecherRemoved { peer_id } => {
                                debug!("Leecher removed: {peer_id:?}");
                                self.state.on_leecher_removed(&peer_id).await;
                                if self.state.role(&peer_id).await == Some(ConnectedPeerRole::Transient) {
                                    self.kam_mut().stop_leeching(peer_id);
                                }
                            },
                            KamilataEvent::SeederRemoved { peer_id } => {
                                debug!("Seeder removed: {peer_id:?}");
                                self.state.on_seeder_removed(&peer_id).await;
                            },
                        },
                        SwarmEvent::NewListenAddr { listener_id, address } => debug!("Listening on {address:?} (listener id: {listener_id:?})"),
                        SwarmEvent::ConnectionEstablished { peer_id, endpoint, num_established, .. } => debug!("Connection established with {peer_id:?} (num_established: {num_established:?}, endpoint: {endpoint:?})"),
                        SwarmEvent::ConnectionClosed { peer_id, endpoint, num_established, .. } => debug!("Connection closed with {peer_id:?} (num_established: {num_established:?}, endpoint: {endpoint:?})"),
                        SwarmEvent::OutgoingConnectionError { peer_id, error } => debug!("Outgoing connection error to {peer_id:?}: {error:?}"),
                        SwarmEvent::ExpiredListenAddr { listener_id, address } => debug!("Expired listen addr {address:?} (listener id: {listener_id:?})"),
                        SwarmEvent::ListenerClosed { listener_id, addresses, reason } => debug!("Listener closed (listener id: {listener_id:?}, addresses: {addresses:?}, reason: {reason:?})"),
                        SwarmEvent::ListenerError { listener_id, error } => debug!("Listener error (listener id: {listener_id:?}, error: {error:?})"),
                        SwarmEvent::Dialing(peer_id) => debug!("Dialing {peer_id:?}"),
                        _ => (),
                    },
                }
            }
        });
        controller
    }
}

enum ClientCommand {
    Search {
        queries: SearchQueries,
        config: SearchConfig,
        sender: OneshotSender<OngoingSearchController<DocumentResult>>,
    },
    Dial {
        addr: Multiaddr,
    },
    LeechFromAll,
}

pub struct KamilataController {
    sender: Sender<ClientCommand>,
    state: Arc<KamilataState>,
}

impl KamilataController {
    pub async fn search(&self, queries: SearchQueries) -> OngoingSearchController<DocumentResult> {
        let (sender, receiver) = oneshot_channel();
        let _ = self.sender.send(ClientCommand::Search {
            queries,
            config: SearchConfig::default(),
            sender,
        }).await;
        receiver.await.unwrap()
    }

    pub async fn dial(&self, addr: Multiaddr) {
        let _ = self.sender.send(ClientCommand::Dial {
            addr,
        }).await;
    }

    pub async fn leech_from_all(&self) {
        let _ = self.sender.send(ClientCommand::LeechFromAll).await;
    }
}
