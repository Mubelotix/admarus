use crate::prelude::*;
use kamilata::behaviour::KamilataEvent;
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
    discovery: DiscoveryBehavior,
}

#[derive(Debug)]
enum Event {
    Identify(Box<IdentifyEvent>),
    Kamilata(KamilataEvent),
    Discovery(DiscoveryEvent),
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

impl From<DiscoveryEvent> for Event {
    fn from(event: DiscoveryEvent) -> Self {
        Self::Discovery(event)
    }
}

pub struct KamilataNode {
    swarm: Swarm<AdmarusBehaviour>,
    swarm_manager: Arc<SwarmManager>,
}

impl KamilataNode {
    pub async fn init(config: Arc<Args>, index: DocumentIndex<FILTER_SIZE>) -> KamilataNode {
        let local_key = Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        let swarm_manager = Arc::new(SwarmManager::new(Arc::clone(&config)));
        let swarm_manager2 = Arc::clone(&swarm_manager);
        let approve_leecher = move |peer_id: PeerId| -> Pin<Box<dyn Future<Output = bool> + Send>> {
            let swarm_manager3 = Arc::clone(&swarm_manager2);
            Box::pin(async move {
                swarm_manager3.second_class_slot_available().await || swarm_manager3.class(&peer_id).await == Some(PeerClass::First)
            })
        };

        let kamilata = KamilataBehaviour::new_with_config_and_store(peer_id, KamilataConfig {
            approve_leecher: Some(Box::new(approve_leecher)),
            protocol_names: vec![String::from("/admarus/kamilata/0.1.0")],
            ..KamilataConfig::default()
        }, index);
        let identify = IdentifyBehaviour::new(
            IdentifyConfig::new(String::from("admarus/0.1.0"), local_key.public())
        );
        let discovery = DiscoveryBehavior::new_with_config(DiscoveryConfig {
            default_visibility: true,
            ..DiscoveryConfig::default()
        });
        let behaviour = AdmarusBehaviour {
            kamilata,
            identify,
            discovery,
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
            swarm_manager,
        }
    }

    fn kam_mut(&mut self) -> &mut KamilataBehaviour<FILTER_SIZE, DocumentIndex<FILTER_SIZE>> {
        &mut self.swarm.behaviour_mut().kamilata
    }

    fn disc_mut(&mut self) -> &mut DiscoveryBehavior {
        &mut self.swarm.behaviour_mut().discovery
    }

    pub fn run(mut self) -> KamilataController {
        let (sender, mut receiver) = channel(1);
        let controller = 
        KamilataController {
            sender,
            swarm_manager: Arc::clone(&self.swarm_manager)
        };
        tokio::spawn(async move {
            loop {
                let recv = Box::pin(receiver.recv());
                let value = futures::future::select(recv, self.swarm.select_next_some()).await;
                match value {
                    // Client commands
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
                                trace!("Leeching from {peer_id}");
                                self.kam_mut().leech_from(peer_id);
                            }
                        },
                    },
                    future::Either::Left((None, _)) => break,
                    future::Either::Right((event, _)) => match event {
                        // Identify events
                        SwarmEvent::Behaviour(Event::Identify(event)) => match *event {
                            IdentifyEvent::Received { peer_id, info } => {
                                trace!("Received identify info from {peer_id}: {info:?}");
                                let r = self.kam_mut().set_addresses(&peer_id, info.listen_addrs.clone()).await;
                                if let Err(e) = r {
                                    error!("Error while setting addresses for {peer_id}: {e:?}");
                                }
                                self.disc_mut().set_info(peer_id, info).await;
                            },
                            IdentifyEvent::Sent { peer_id } => trace!("Sent identify info to {peer_id}"),
                            IdentifyEvent::Pushed { peer_id } => trace!("Pushed identify info to {peer_id}"),
                            IdentifyEvent::Error { peer_id, error } => debug!("Identify error with {peer_id}: {error}"),
                        },
                        // Kamilata events
                        SwarmEvent::Behaviour(Event::Kamilata(event)) => match event {
                            KamilataEvent::LeecherAdded { peer_id, filter_count, interval_ms } => {
                                debug!("Leecher added: {peer_id} (filter_count: {filter_count}, interval_ms: {interval_ms})");
                                let r = self.swarm_manager.on_leecher_added(peer_id).await;
                                if let Err(e) = r {
                                    error!("Error while adding leecher {peer_id}: {e:?}");
                                    self.kam_mut().stop_seeding(peer_id);
                                } else if self.swarm_manager.class(&peer_id).await == Some(PeerClass::Second) {
                                    self.kam_mut().leech_from(peer_id);
                                }
                            },
                            KamilataEvent::SeederAdded { peer_id } => {
                                debug!("Seeder added: {peer_id}");
                                self.swarm_manager.on_seeder_added(peer_id).await;
                            },
                            KamilataEvent::LeecherRemoved { peer_id } => {
                                debug!("Leecher removed: {peer_id}");
                                self.swarm_manager.on_leecher_removed(&peer_id).await;
                                if self.swarm_manager.class(&peer_id).await == Some(PeerClass::Transient) {
                                    self.kam_mut().stop_leeching(peer_id);
                                }
                            },
                            KamilataEvent::SeederRemoved { peer_id } => {
                                debug!("Seeder removed: {peer_id}");
                                self.swarm_manager.on_seeder_removed(&peer_id).await;
                            },
                        },
                        SwarmEvent::NewListenAddr { listener_id, address } => debug!("Listening on {address} (listener id: {listener_id:?})"),
                        SwarmEvent::ConnectionEstablished { peer_id, endpoint, num_established, .. } => {
                            debug!("Connection established with {peer_id} (num_established: {num_established}, endpoint: {endpoint:?})");
                            self.swarm_manager.on_peer_connected(peer_id).await;
                        },
                        SwarmEvent::ConnectionClosed { peer_id, num_established, .. } => {
                            if num_established == 0 {
                                debug!("Peer {peer_id} disconnected");
                                self.swarm_manager.on_peer_disconnected(&peer_id).await;
                            }
                        },
                        SwarmEvent::OutgoingConnectionError { peer_id, error } => debug!("Outgoing connection error to {peer_id:?}: {error}"),
                        SwarmEvent::ExpiredListenAddr { listener_id, address } => debug!("Expired listen addr {address} (listener id: {listener_id:?})"),
                        SwarmEvent::ListenerClosed { listener_id, addresses, reason } => debug!("Listener closed (listener id: {listener_id:?}, addresses: {addresses:?}, reason: {reason:?})"),
                        SwarmEvent::ListenerError { listener_id, error } => debug!("Listener error (listener id: {listener_id:?}, error: {error})"),
                        SwarmEvent::Dialing(peer_id) => debug!("Dialing {peer_id}"),
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
    swarm_manager: Arc<SwarmManager>,
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
