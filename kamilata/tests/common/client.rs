
use futures::future;
use libp2p::{identity::{self, Keypair}, core::transport::MemoryTransport, PeerId, Transport, Swarm, Multiaddr, swarm::{SwarmEvent, SwarmBuilder}};

use tokio::sync::{
    mpsc::*,
    oneshot::{
        channel as oneshot_channel, Sender as OneshotSender,
    }
};
use futures::StreamExt;
use log::*;
use super::*;

pub fn memory_transport(
    keypair: identity::Keypair,
) -> std::io::Result<libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>> {
    Ok(MemoryTransport::default()
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(libp2p::noise::Config::new(&keypair).expect("signing libp2p-noise static keypair"))
        .multiplex(libp2p::yamux::Config::default())
        .timeout(std::time::Duration::from_secs(20))
        .boxed())
}

pub struct Client {
    local_key: Keypair,
    local_peer_id: PeerId,
    swarm: Swarm<KamilataBehaviour<125000, MovieIndex<125000>>>,
    addr: Multiaddr,
}

#[derive(Debug)]
pub enum ClientCommand {
    Dial {
        addr: Multiaddr,
    },
    Search {
        query: MovieQuery,
        sender: OneshotSender<SearchResults<Movie>>,
        config: SearchConfig,
    },
    GetRoutingStats {
        sender: OneshotSender<(usize, usize)>,
    },
    LeechFrom {
        seeder: PeerId,
    },
    LeechFromAll,
}

pub struct ClientController {
    sender: Sender<ClientCommand>,
    peer_id: PeerId,
}

impl ClientController {
    pub async fn dial(&self, addr: Multiaddr) {
        self.sender.send(ClientCommand::Dial { addr }).await.unwrap();
    }

    pub async fn leech_from(&self, seeder: &ClientController) {
        self.sender.send(ClientCommand::LeechFrom { seeder: seeder.peer_id }).await.unwrap();
    }

    pub async fn leech_from_all(&self) {
        self.sender.send(ClientCommand::LeechFromAll).await.unwrap();
    }

    pub async fn search(&self, query: impl Into<MovieQuery>) -> SearchResults<Movie> {
        self.search_with_config(query, SearchConfig::default()).await
    }

    pub async fn search_with_priority(&self, query: impl Into<MovieQuery>, priority: SearchPriority) -> SearchResults<Movie> {
        self.search_with_config(query, SearchConfig::default().with_priority(priority)).await
    }

    pub async fn search_with_config(&self, query: impl Into<MovieQuery>, config: SearchConfig) -> SearchResults<Movie> {
        let (sender, receiver) = oneshot_channel();
        self.sender.send(ClientCommand::Search {
            query: query.into(),
            sender,
            config,
        }).await.unwrap();
        receiver.await.unwrap()
    }

    /// Returns (seeder_count, leecher_count)
    pub async fn get_routing_stats(&self) -> (usize, usize) {
        let (sender, receiver) = oneshot_channel();
        self.sender.send(ClientCommand::GetRoutingStats {
            sender,
        }).await.unwrap();
        receiver.await.unwrap()
    }
}

impl Client {
    pub async fn init() -> Self {
        Self::init_with_config(KamilataConfig::default()).await
    }

    pub async fn init_with_config(config: KamilataConfig) -> Self {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
    
        let transport = memory_transport(local_key.clone()).unwrap();

        // Create a ping network behaviour.
        //
        // For illustrative purposes, the ping protocol is configured to
        // keep the connection alive, so a continuous sequence of pings
        // can be observed.
        let behaviour = KamilataBehaviour::new_with_config(local_peer_id, config);
    
        let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();
    
        // Tell the swarm to listen on all interfaces and a random, OS-assigned port.
        let mut addr: Option<Multiaddr> = None;
        for _ in 0..20 {
            let n: usize = rand::random();
            let addr2: Multiaddr = format!("/memory/{n}").parse().unwrap();
            match swarm.listen_on(addr2.clone()) {
                Ok(_) => {
                    addr = Some(addr2);
                    break;
                }
                Err(err) => eprintln!("Failed to listen on {addr2} {err}"),
            }
        }
    
        Client {
            local_key,
            local_peer_id,
            swarm,
            addr: addr.expect("Failed to listen on any addr"),
        }
    }

    pub fn addr(&self) -> &Multiaddr {
        &self.addr
    }

    pub fn peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    pub fn behaviour(&self) -> &KamilataBehaviour<125000, MovieIndex<125000>> {
        self.swarm.behaviour()
    }

    pub fn behaviour_mut(&mut self) -> &mut KamilataBehaviour<125000, MovieIndex<125000>> {
        self.swarm.behaviour_mut()
    }

    pub fn store(&self) -> &MovieIndex<125000> {
        self.swarm.behaviour().store()
    }

    pub fn swarm(&self) -> &Swarm<KamilataBehaviour<125000, MovieIndex<125000>>> {
        &self.swarm
    }

    pub fn swarm_mut(&mut self) -> &mut Swarm<KamilataBehaviour<125000, MovieIndex<125000>>> {
        &mut self.swarm
    }

    pub fn run(mut self) -> ClientController {
        let (sender, mut receiver) = channel(1);
        tokio::spawn(async move {
            loop {
                let recv = Box::pin(receiver.recv());
                let value = futures::future::select(recv, self.swarm.select_next_some()).await;
                match value {
                    future::Either::Left((Some(command), _)) => match command {
                        ClientCommand::Dial { addr } => {
                            self.swarm.dial(addr).unwrap();
                        },
                        ClientCommand::LeechFrom { seeder } => {
                            self.swarm.behaviour_mut().leech_from(seeder);
                        },
                        ClientCommand::LeechFromAll => {
                            let peer_ids = self.swarm.connected_peers().cloned().collect::<Vec<_>>();
                            for peer_id in peer_ids {
                                self.swarm.behaviour_mut().leech_from(peer_id);
                            }
                        },
                        ClientCommand::Search { query: queries, sender, config } => {
                            let mut controler = self.swarm.behaviour_mut().search_with_config(queries, config).await;
                    
                            tokio::spawn(async move {
                                let mut hits = Vec::new();
                                while let Some(hit) = controler.recv().await {
                                    hits.push(hit);
                                }
                                let mut results = controler.finish().await;
                                results.hits = hits;
                                sender.send(results).unwrap();
                            });
                        },
                        ClientCommand::GetRoutingStats { sender } => {
                            let seeder_count = self.swarm.behaviour_mut().seeder_count().await;
                            let leecher_count = self.swarm.behaviour_mut().leecher_count().await;
                            sender.send((seeder_count, leecher_count)).unwrap();    
                        }
                    },
                    future::Either::Left((None, _)) => break,
                    future::Either::Right((event, _)) => match event {
                        SwarmEvent::Behaviour(e) => info!("{} produced behaviour event {e:?}", self.local_peer_id),
                        SwarmEvent::NewListenAddr { listener_id, address } => debug!("{} is listening on {address:?} (listener id: {listener_id:?})", self.local_peer_id),
                        _ => ()
                    },
                }
            }
        });
        ClientController {
            sender,
            peer_id: self.local_peer_id,
        }
    }
}
