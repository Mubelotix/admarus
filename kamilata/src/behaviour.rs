use crate::prelude::*;

/// Events produced by the [KamilataBehaviour]
#[derive(Debug)]
pub enum KamilataEvent {
    // TODO unroutable, routable and pending

    /// Sent when we start seeding to a peer.
    LeecherAdded { peer_id: PeerId, filter_count: usize, interval_ms: usize },
    /// Sent when we start leeching from a peer.
    SeederAdded { peer_id: PeerId },
    /// Sent when a seeding task is aborted.
    /// This can happen even if LeecherAdded was not sent.
    LeecherRemoved { peer_id: PeerId },
    /// Sent when a leeching task is aborted.
    /// This can happen even if SeederAdded was not sent.
    SeederRemoved { peer_id: PeerId },
}

/// Implementation of the Kamilata protocol.
/// 
/// # Peer Discovery
/// 
/// The [KamilataBehaviour] does not provide peer discovery by itself.
/// Peer discovery is the process by which peers in a p2p network exchange information about each other among other reasons to become resistant against the failure or replacement of the boot nodes of the network.
/// Furthermore, the [KamilataBehaviour] does not reimplement the capabilities of libp2p's [Identify](libp2p::identify::Behaviour).
/// As a result, Kamilata only infers listen addresses of the peers we successfully dialed.
/// This means that the [Identify](libp2p::identify::Behaviour) protocol must be manually hooked up to Kademlia through calls to [KamilataBehaviour::add_address].
/// If you choose not to use libp2p's [Identify](libp2p::identify::Behaviour), incoming connections will be accepted but we won't be able to relay queries to them.
/// This is the same approach as [Kademlia](libp2p::kad::Kademlia).
pub struct KamilataBehaviour<const N: usize, S: Store<N>> {
    our_peer_id: PeerId,
    connections: HashMap<PeerId, isize>,
    db: Arc<Db<N, S>>,
    config: Arc<KamilataConfig>,

    rt_handle: tokio::runtime::Handle,

    /// Used to create new [BehaviourController]s
    control_msg_sender: Sender<BehaviourControlMessage<N, S>>,
    /// Receiver of messages from [BehaviourController]s
    control_msg_receiver: Receiver<BehaviourControlMessage<N, S>>,
    /// When a message is to be sent to a handler that is being dialed, it is temporarily stored here.
    pending_handler_events: BTreeMap<PeerId, BehaviorToHandlerEvent<N, S>>,
    /// When a message is ready to be dispatched to a handler, it is moved here.
    handler_event_queue: Vec<(PeerId, BehaviorToHandlerEvent<N, S>)>,

    task_counter: Counter,
    /// Tasks associated with task identifiers.  
    /// Reserved IDs:
    ///     none
    tasks: HashMap<usize, Task>,
}

impl<const N: usize, S: Store<N> + Default> KamilataBehaviour<N, S> {
    pub fn new(our_peer_id: PeerId) -> KamilataBehaviour<N, S> {
        Self::new_with_config(our_peer_id, KamilataConfig::default())
    }

    pub fn new_with_config(our_peer_id: PeerId, config: KamilataConfig) -> KamilataBehaviour<N, S> {
        let rt_handle = tokio::runtime::Handle::current();
        let (control_msg_sender, control_msg_receiver) = channel(100);
        let db_behaviour_controller = BehaviourController {
            sender: control_msg_sender.clone(),
        };
        let config = Arc::new(config);

        KamilataBehaviour {
            our_peer_id,
            connections: HashMap::new(),
            db: Arc::new(Db::new(Arc::clone(&config), S::default(), db_behaviour_controller)),
            config,
            control_msg_sender,
            control_msg_receiver,
            pending_handler_events: BTreeMap::new(),
            handler_event_queue: Vec::new(),
            rt_handle,
            task_counter: Counter::new(0),
            tasks: HashMap::new(),
        }
    }
}

impl<const N: usize, S: Store<N>> KamilataBehaviour<N, S> {
    pub fn new_with_store(our_peer_id: PeerId, store: S) -> KamilataBehaviour<N, S> {
        Self::new_with_config_and_store(our_peer_id, KamilataConfig::default(), store)
    }

    pub fn new_with_config_and_store(our_peer_id: PeerId, config: KamilataConfig, store: S) -> KamilataBehaviour<N, S> {
        let rt_handle = tokio::runtime::Handle::current();
        let (control_msg_sender, control_msg_receiver) = channel(100);
        let db_behaviour_controller = BehaviourController {
            sender: control_msg_sender.clone(),
        };
        let config = Arc::new(config);

        KamilataBehaviour {
            our_peer_id,
            connections: HashMap::new(),
            db: Arc::new(Db::new(Arc::clone(&config), store, db_behaviour_controller)),
            config,
            control_msg_sender,
            control_msg_receiver,
            pending_handler_events: BTreeMap::new(),
            handler_event_queue: Vec::new(),
            rt_handle,
            task_counter: Counter::new(0),
            tasks: HashMap::new(),
        }
    }

    pub async fn get_config(&self) -> Arc<KamilataConfig> {
        Arc::clone(&self.config)
    }

    pub fn store(&self) -> &S {
        self.db.store()
    }

    pub async fn seeder_count(&self) -> usize {
        self.db.seeder_count().await
    }

    fn new_controller(&self) -> BehaviourController<N, S> {
        BehaviourController {
            sender: self.control_msg_sender.clone(),
        }
    }

    pub async fn leecher_count(&self) -> usize {
        self.db.leecher_count().await
    }

    /// Starts leeching from a peer.
    /// If we already leech from this peer, this function does nothing.
    pub fn leech_from(&mut self, seeder: PeerId) {
        self.handler_event_queue.push((seeder, BehaviorToHandlerEvent::LeechFilters));
    }

    pub fn stop_leeching(&mut self, seeder: PeerId) {
        self.handler_event_queue.push((seeder, BehaviorToHandlerEvent::StopLeeching));
    }

    pub fn stop_seeding(&mut self, seeder: PeerId) {
        self.handler_event_queue.push((seeder, BehaviorToHandlerEvent::StopSeeding));
    }

    /// Starts a new search and returns an [handler](OngoingSearchControler) to control it.
    pub async fn search(&mut self, query: impl Into<S::Query>) -> OngoingSearchController<N, S> {
        self.search_with_config(query, SearchConfig::default()).await
    }

    /// Starts a new search with custom [SearchPriority] and returns an [handler](OngoingSearchControler) to control it.
    pub async fn search_with_priority(&mut self, query: impl Into<S::Query>, priority: SearchPriority) -> OngoingSearchController<N, S> {
        self.search_with_config(query, SearchConfig::default().with_priority(priority)).await
    }

    /// Starts a new search with custom [SearchConfig] and returns an [handler](OngoingSearchControler) to control it.
    pub async fn search_with_config(&mut self, query: impl Into<S::Query>, config: SearchConfig) -> OngoingSearchController<N, S> {
        let query = query.into();
        let search_state = OngoingSearchState::new(query, config);
        let (search_controler, search_follower) = search_state.into_pair();
        self.tasks.insert(self.task_counter.next() as usize, Box::pin(search(search_follower, self.new_controller(), Arc::clone(&self.db), self.our_peer_id)));
        search_controler
    }

    /// Adds a known listen address of a peer participating in the network.
    /// Returns an error if the peer is not connected to us.
    /// 
    /// This function is inspired by [Kademlia::add_address](libp2p::kad::Kademlia::add_address).  
    /// It is preferred to use [Kamilata::set_addresses] instead, as it retains meaning from the order of the addresses (ordered from the most reliable).
    pub async fn add_address(&mut self, peer: &PeerId, address: Multiaddr) -> Result<(), DisconnectedPeer> {
        self.db.add_address(*peer, address, true).await
    }

    /// Sets the known listen addresses of a peer participating in the network.
    /// Returns an error if the peer is not connected to us.
    pub async fn set_addresses(&mut self, peer: &PeerId, addresses: Vec<Multiaddr>) -> Result<(), DisconnectedPeer> {
        self.db.set_addresses(*peer, addresses).await
    }
}

impl<const N: usize, S: Store<N>> NetworkBehaviour for KamilataBehaviour<N, S> {
    type ConnectionHandler = KamilataHandler<N, S>;
    type ToSwarm = KamilataEvent;

    fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
        match event {
            FromSwarm::ConnectionEstablished(info) => {
                self.connections.entry(info.peer_id).and_modify(|count| *count += 1).or_insert(1);
                if let Some(msg) = self.pending_handler_events.remove(&info.peer_id) {
                    self.handler_event_queue.push((info.peer_id, msg));
                }
                let addrs = if let ConnectedPoint::Dialer { address, .. } = info.endpoint {
                    vec![address.to_owned()]
                } else {
                    Vec::new()
                };
                let db2 = Arc::clone(&self.db);
                let peer_id = info.peer_id;
                tokio::spawn(async move {
                    db2.add_peer(peer_id, addrs).await;
                });
            },
            FromSwarm::DialFailure(info) => {
                if let Some(peer_id) = info.peer_id {
                    self.pending_handler_events.remove(&peer_id);
                }
                warn!("{} Dial failure: {} with {:?}", self.our_peer_id, info.error, info.peer_id);
            },
            FromSwarm::ConnectionClosed(info) => {
                let peer_connections = *self.connections.entry(info.peer_id).and_modify(|count| *count -= 1).or_default();
                self.connections.retain(|_, count| *count > 0);
                if peer_connections <= 0 {
                    self.handler_event_queue.retain(|(peer_id, _)| peer_id != &info.peer_id);
                    self.pending_handler_events.remove(&info.peer_id);
                    let db2 = Arc::clone(&self.db);
                    tokio::spawn(async move {
                        db2.remove_peer(&info.peer_id).await;
                    });
                }
            },
            _ => ()
        }
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        match event {

        }
    }

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        remote_peer_id: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(KamilataHandler::new(self.our_peer_id, remote_peer_id, Arc::clone(&self.db), Arc::clone(&self.config)))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        peer: PeerId,
        _addr: &Multiaddr,
        _role_override: Endpoint,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(KamilataHandler::new(self.our_peer_id, peer, Arc::clone(&self.db), Arc::clone(&self.config)))
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>> {
        // Message handlers first
        if let Some((peer_id, event)) = self.handler_event_queue.pop() {
            return Poll::Ready(
                ToSwarm::NotifyHandler {
                    peer_id,
                    handler: libp2p::swarm::NotifyHandler::Any,
                    event
                }
            );
        }
        if let Poll::Ready(Some(control_message)) = self.control_msg_receiver.poll_recv(cx) {
            match control_message {
                BehaviourControlMessage::OutputEvent(event) => {
                    return Poll::Ready(
                        ToSwarm::GenerateEvent(event)
                    );
                }
                BehaviourControlMessage::DialPeerAndMessage(peer_id, addresses, event) => {
                    // Just notify the handler directly if we are already connected to the peer.
                    trace!("{} Dialing peer {peer_id} with addresses {addresses:?} and sending message", self.our_peer_id);
                    if self.connections.get(&peer_id).unwrap_or(&0) > &0 {
                        return Poll::Ready(
                            ToSwarm::NotifyHandler {
                                peer_id,
                                handler: libp2p::swarm::NotifyHandler::Any,
                                event
                            }
                        );
                    }
                    self.pending_handler_events.insert(peer_id, event);
                    return Poll::Ready(
                        ToSwarm::Dial {
                            opts: libp2p::swarm::dial_opts::DialOpts::peer_id(peer_id).addresses(addresses).build(),
                        }
                    );
                }
            }
        }

        // It seems this method gets called in a context where the tokio runtime does not exist.
        // We import that runtime so that we can rely on it.
        let _rt_enter_guard = self.rt_handle.enter();

        // Poll tasks
        for tid in self.tasks.keys().copied().collect::<Vec<_>>() {
            let task = self.tasks.get_mut(&tid).unwrap();

            match task.poll_unpin(cx) {
                Poll::Ready(output) => {
                    trace!("{} Task {tid} completed!", self.our_peer_id);
                    self.tasks.remove(&tid);

                    match output {
                        TaskOutput::None => (),
                    }
                }
                Poll::Pending => ()
            }
        }
        
        Poll::Pending
    }
}

/// Internal control messages send by [BehaviourController] to [KamilataBehaviour]
#[derive(Debug)]
pub(crate) enum BehaviourControlMessage<const N: usize, S: Store<N>> {
    DialPeerAndMessage(PeerId, Vec<Multiaddr>, BehaviorToHandlerEvent<N, S>),
    OutputEvent(KamilataEvent),
}

/// A struct that allows to send messages to an [handler](ConnectionHandler)
pub(crate) struct BehaviourController<const N: usize, S: Store<N>> {
    sender: Sender<BehaviourControlMessage<N, S>>,
}

impl<const N: usize, S: Store<N>> Clone for BehaviourController<N, S> {
    fn clone(&self) -> Self {
        Self { sender: self.sender.clone() }
    }
}

impl<const N: usize, S: Store<N>> BehaviourController<N, S> {
    /// Requests behaviour to dial a peer and send a message to it.
    pub async fn dial_peer_and_message(&self, peer_id: PeerId, addresses: Vec<Multiaddr>, message: BehaviorToHandlerEvent<N, S>) {
        if let Err(e) = self.sender.send(BehaviourControlMessage::DialPeerAndMessage(peer_id, addresses, message)).await {
            error!("Failed to dial peer and message {e}");
        }
    }

    /// Outputs an event from the behaviour.
    pub async fn emit_event(&self, event: KamilataEvent) {
        if let Err(e) = self.sender.send(BehaviourControlMessage::OutputEvent(event)).await {
            error!("Failed to emit event {e}");
        }
    }
}
