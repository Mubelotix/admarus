use crate::prelude::*;

pub const FILTER_SIZE: usize = 125000;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "Event")]
struct AdmarusBehaviour {
    kamilata: KamilataBehaviour<FILTER_SIZE, DocumentIndex>,
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

pub struct Node {
    swarm: Swarm<AdmarusBehaviour>,
    sw: Arc<SwarmManager>,
}

impl Node {
    pub async fn init(config: Arc<Args>, index: DocumentIndex) -> (Node, Keypair) {
        let keypair = Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        info!("Local peer id: {peer_id}");

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
            IdentifyConfig::new(String::from("admarus/0.1.0"), keypair.public())
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
                
        let mut swarm = SwarmBuilder::with_existing_identity(keypair.clone())
            .with_tokio()
            .with_tcp(
                Default::default(),
                (libp2p_tls::Config::new, libp2p_noise::Config::new),
                libp2p_yamux::Config::default,
            )
            .expect("Failed to build swarm with transport")
            .with_behaviour(|_| behaviour)
            .expect("Failed to build swarm with behaviour")
            .build();
        for listen_addr in &config.listen_addrs {
            let Ok(parsed_addr) = listen_addr.parse::<Multiaddr>() else {
                error!("Invalid address: {listen_addr}");
                continue;
            };
            match swarm.listen_on(parsed_addr.clone()) {
                Ok(_listerner_id) => (),
                Err(e) => error!("Could not listen on {listen_addr}: {e:?}"),
            }
        }

        (Node {
            swarm,
            sw: swarm_manager,
        }, keypair)
    }

    fn kam_mut(&mut self) -> &mut KamilataBehaviour<FILTER_SIZE, DocumentIndex> {
        &mut self.swarm.behaviour_mut().kamilata
    }

    fn disc_mut(&mut self) -> &mut DiscoveryBehavior {
        &mut self.swarm.behaviour_mut().discovery
    }

    pub fn run(mut self) -> NodeController {
        let (sender, mut receiver) = channel(1);
        let controller = NodeController {
            peer_id: *self.swarm.local_peer_id(),
            sender,
            sw: Arc::clone(&self.sw)
        };
        tokio::spawn(async move {
            loop {
                let recv = Box::pin(receiver.recv());
                let value = futures::future::select(recv, self.swarm.select_next_some()).await;
                match value {
                    // Client commands
                    Either::Left((Some(command), _)) => match command {
                        ClientCommand::Search { query: queries, config, sender } => {
                            let controller = self.kam_mut().search_with_config(queries, config).await;
                            let _ = sender.send(controller);
                        },
                        ClientCommand::GetExternalAddrs { sender } => {
                            let addrs = self.swarm.external_addresses();
                            let _ = sender.send(addrs.cloned().collect());
                        },
                        ClientCommand::QueryPeers { query: q, sender } => {
                            self.disc_mut().start_query(q, sender);
                        }
                        ClientCommand::Dial(dial_opts) => {
                            let r = self.swarm.dial(dial_opts);
                            if let Err(e) = r {
                                error!("Error while dialing: {e:?}");
                            }
                        },
                        ClientCommand::Disconnect { peer_id } => {
                            let r = self.swarm.disconnect_peer_id(peer_id);
                            if let Err(e) = r {
                                error!("Error while disconnecting from {peer_id}: {e:?}");
                            }
                        },
                        ClientCommand::LeechFrom(peer_id) => {
                            trace!("Leeching from {peer_id}");
                            self.kam_mut().leech_from(peer_id);
                        },
                    },
                    Either::Left((None, _)) => break,
                    Either::Right((event, _)) => match event {
                        // Identify events
                        SwarmEvent::Behaviour(Event::Identify(event)) => match *event {
                            IdentifyEvent::Received { peer_id, info } => {
                                trace!("Received identify info from {peer_id}: {info:?}");
                                let r = self.kam_mut().set_addresses(&peer_id, info.listen_addrs.clone()).await;
                                if let Err(e) = r {
                                    error!("Error while setting addresses for {peer_id}: {e:?}");
                                }
                                self.disc_mut().set_info(peer_id, info.clone()).await;
                                self.sw.on_identify(&peer_id, info).await;
                            },
                            IdentifyEvent::Sent { peer_id } => trace!("Sent identify info to {peer_id}"),
                            IdentifyEvent::Pushed { peer_id, info } => trace!("Pushed identify info {info:?} to {peer_id}"),
                            IdentifyEvent::Error { peer_id, error } => debug!("Identify error with {peer_id}: {error}"),
                        },
                        // Kamilata events
                        SwarmEvent::Behaviour(Event::Kamilata(event)) => match event {
                            KamilataEvent::LeecherAdded { peer_id, filter_count, interval_ms } => {
                                debug!("Leecher added: {peer_id} (filter_count: {filter_count}, interval_ms: {interval_ms})");
                                let r = self.sw.on_leecher_added(peer_id).await;
                                if let Err(e) = r {
                                    error!("Error while adding leecher {peer_id}: {e:?}");
                                    self.kam_mut().stop_seeding(peer_id);
                                } else if self.sw.class(&peer_id).await == Some(PeerClass::Second) {
                                    self.kam_mut().leech_from(peer_id);
                                }
                            },
                            KamilataEvent::SeederAdded { peer_id } => {
                                debug!("Seeder added: {peer_id}");
                                self.sw.on_seeder_added(peer_id).await;
                            },
                            KamilataEvent::LeecherRemoved { peer_id } => {
                                debug!("Leecher removed: {peer_id}");
                                self.sw.on_leecher_removed(&peer_id).await;
                                if self.sw.class(&peer_id).await == Some(PeerClass::Transient) {
                                    self.kam_mut().stop_leeching(peer_id);
                                }
                            },
                            KamilataEvent::SeederRemoved { peer_id } => {
                                debug!("Seeder removed: {peer_id}");
                                self.sw.on_seeder_removed(&peer_id).await;
                            },
                        },
                        // Discovery events
                        SwarmEvent::Behaviour(Event::Discovery(event)) => match event {

                        }
                        SwarmEvent::NewListenAddr { listener_id, address } => debug!("Listening on {address} (listener id: {listener_id:?})"),
                        SwarmEvent::ConnectionEstablished { peer_id, endpoint, num_established, .. } => {
                            debug!("Connection established with {peer_id} (num_established: {num_established}, endpoint: {endpoint:?})");
                            self.sw.on_peer_connected(peer_id).await;
                        },
                        SwarmEvent::ConnectionClosed { peer_id, num_established, .. } => {
                            if num_established == 0 {
                                debug!("Peer {peer_id} disconnected");
                                self.sw.on_peer_disconnected(&peer_id).await;
                            }
                        },
                        SwarmEvent::OutgoingConnectionError { peer_id, connection_id, error } => debug!("Outgoing connection {connection_id:?} error to {peer_id:?}: {error}"),
                        SwarmEvent::ExpiredListenAddr { listener_id, address } => debug!("Expired listen addr {address} (listener id: {listener_id:?})"),
                        SwarmEvent::ListenerClosed { listener_id, addresses, reason } => debug!("Listener closed (listener id: {listener_id:?}, addresses: {addresses:?}, reason: {reason:?})"),
                        SwarmEvent::ListenerError { listener_id, error } => debug!("Listener error (listener id: {listener_id:?}, error: {error})"),
                        SwarmEvent::Dialing { peer_id, connection_id } => debug!("Dialing {peer_id:?} ({connection_id:?})"),
                        SwarmEvent::IncomingConnection { connection_id, local_addr, send_back_addr } => trace!("Incoming connection from {send_back_addr} (local addr: {local_addr}, connection id: {connection_id:?})"),
                        SwarmEvent::IncomingConnectionError { connection_id, local_addr, send_back_addr, error } => trace!("Incoming connection error from {send_back_addr} (local addr: {local_addr}, connection id: {connection_id:?}, error: {error})"),
                        other => trace!("Unknown swarm event: {other:?}"),
                    },
                }
            }
        });
        controller
    }
}

enum ClientCommand {
    Search {
        query: Query,
        config: SearchConfig,
        sender: OneshotSender<OngoingSearchController<FILTER_SIZE, DocumentIndex>>,
    },
    GetExternalAddrs {
        sender: OneshotSender<Vec<Multiaddr>>,
    },
    QueryPeers {
        query: PeerListQuery,  
        sender: OneshotSender<Result<DiscoveryResponse, IoError>>,
    },
    Dial(DialOpts),
    Disconnect {
        peer_id: PeerId,
    },
    LeechFrom(PeerId),
}

#[derive(Clone)]
pub struct NodeController {
    peer_id: PeerId,
    sender: Sender<ClientCommand>,
    pub sw: Arc<SwarmManager>,
}

impl NodeController {
    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub async fn search(&self, query: Query) -> OngoingSearchController<FILTER_SIZE, DocumentIndex> {
        let (sender, receiver) = oneshot_channel();
        let _ = self.sender.send(ClientCommand::Search {
            query,
            config: SearchConfig::default(),
            sender,
        }).await;
        receiver.await.expect("Channel closed")
    }

    pub async fn external_addresses(&self) -> Vec<Multiaddr> {
        let (sender, receiver) = oneshot_channel();
        let _ = self.sender.send(ClientCommand::GetExternalAddrs {
            sender,
        }).await;
        receiver.await.expect("Channel closed")
    }

    pub async fn dial_with_peer_id(&self, peer_id: PeerId, addrs: Vec<Multiaddr>) {
        let _ = self.sender.send(ClientCommand::Dial(
            DialOpts::peer_id(peer_id).condition(libp2p::swarm::dial_opts::PeerCondition::Disconnected)
                .addresses(addrs)
                .extend_addresses_through_behaviour()
                .build()
        )).await;
    }

    pub async fn query_peers(&self, query: PeerListQuery) -> Result<DiscoveryResponse, IoError> {
        let (sender, receiver) = oneshot_channel();
        let _ = self.sender.send(ClientCommand::QueryPeers {
            query,
            sender,
        }).await;
        match receiver.await {
            Ok(r) => r,
            Err(_) => Err(IoError::new(std::io::ErrorKind::BrokenPipe, "Couldn't receive response")),
        }
    }

    pub async fn disconnect(&self, peer_id: &PeerId) {
        let _ = self.sender.send(ClientCommand::Disconnect {
            peer_id: *peer_id,
        }).await;
    }

    pub async fn leech_from(&self, peer_id: PeerId) {
        let _ = self.sender.send(ClientCommand::LeechFrom(peer_id)).await;
    }
}
