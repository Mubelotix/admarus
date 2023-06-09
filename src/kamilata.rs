use crate::prelude::*;
use libp2p::{swarm::{Swarm, SwarmBuilder, SwarmEvent}, identity::Keypair, PeerId, tcp, Transport, core::{transport::OrTransport, upgrade}, mplex::MplexConfig, noise::{NoiseConfig, self}, Multiaddr};
use tokio::sync::{mpsc::*, oneshot::{Sender as OneshotSender, channel as oneshot_channel}};
use futures::{StreamExt, future};

const FILTER_SIZE: usize = 125000;

pub struct KamilataNode {
    swarm: Swarm<KamilataBehavior<FILTER_SIZE, DocumentIndex<FILTER_SIZE>>>,
}

impl KamilataNode {
    pub async fn init(addr: String, index: DocumentIndex<FILTER_SIZE>) -> KamilataNode {
        let local_key = Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());

        let behaviour = KamilataBehavior::new_with_store(peer_id, index);
        
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::new());

        let transport = tcp_transport
            .upgrade(upgrade::Version::V1Lazy)
            .authenticate(
                noise::Config::new(&local_key).expect("Signing libp2p-noise static DH keypair failed."),
            )
            .multiplex(MplexConfig::default())
            .boxed();
        
        let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build();
        swarm.listen_on(addr.parse().unwrap()).unwrap();

        KamilataNode {
            swarm,
        }
    }

    pub fn run(mut self) -> KamilataController {
        let (sender, mut receiver) = channel(1);
        tokio::spawn(async move {
            loop {
                let recv = Box::pin(receiver.recv());
                let value = futures::future::select(recv, self.swarm.select_next_some()).await;
                match value {
                    future::Either::Left((Some(command), _)) => match command {
                        ClientCommand::Search { queries, config, sender } => {
                            let controller = self.swarm.behaviour_mut().search_with_config(queries, config).await;
                            let _ = sender.send(controller);
                        },
                        ClientCommand::Dial { addr } => {
                            self.swarm.dial(addr).unwrap();
                        },
                        ClientCommand::LeechFromAll => {
                            let peer_ids = self.swarm.connected_peers().cloned().collect::<Vec<_>>();
                            for peer_id in peer_ids {
                                trace!("Leeching from {:?}", peer_id);
                                self.swarm.behaviour_mut().leech_from(peer_id);
                            }
                        },
                    },
                    future::Either::Left((None, _)) => break,
                    future::Either::Right((event, _)) => match event {
                        SwarmEvent::Behaviour(e) => debug!("Produced behavior event {e:?}"),
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
        KamilataController {
            sender,
        }
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
