use crate::prelude::*;

/// Events aimed at a [KamilataHandler]
pub enum BehaviorToHandlerEvent<const N: usize, S: Store<N>> {
    /// Asks the handler to send a request and receive a response.
    Request {
        /// This request packet will be sent through a new outbound substream.
        request: RequestPacket,
        /// The response will be sent back through this channel.
        sender: OneshotSender<Option<ResponsePacket>>,
    },
    /// Opens a channel
    SearchRequest {
        query: Arc<S::Query>,
        routes_sender: Sender<Vec<Route>>,
        result_sender: OngoingSearchFollower<N, S>,
        over_notifier: OneshotSender<()>,
    },
    /// Asks the handler to leech filters
    LeechFilters,
    /// Asks the handler to stop leeching
    StopLeeching,
    /// Asks the handler to stop seeding
    StopSeeding,
}

impl<const N: usize, S: Store<N>> std::fmt::Debug for BehaviorToHandlerEvent<N, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BehaviorToHandlerEvent::Request { .. } => write!(f, "Request"),
            BehaviorToHandlerEvent::SearchRequest { .. } => write!(f, "SearchRequest"),
            BehaviorToHandlerEvent::LeechFilters => write!(f, "LeechFilters"),
            BehaviorToHandlerEvent::StopLeeching => write!(f, "StopLeeching"),
            BehaviorToHandlerEvent::StopSeeding => write!(f, "StopSeeding"),
        }
    }
}

/// Events produced by a [KamilataHandler] (unused)
#[derive(Debug)]
pub enum HandlerToBehaviorEvent {}

/// The [KamilataHandler] is responsible for handling a connection to a remote peer.
/// Multiple handlers are managed by the [KamilataBehaviour].
pub struct KamilataHandler<const N: usize, S: Store<N>> {
    our_peer_id: PeerId,
    remote_peer_id: PeerId,
    db: Arc<Db<N, S>>,
    config: Arc<KamilataConfig>,

    rt_handle: tokio::runtime::Handle,
    
    task_counter: Counter,
    /// Tasks associated with task identifiers.  
    /// Reserved IDs:
    ///     1: filter seeder
    ///     2: filter leecher
    tasks: HashMap<u32, HandlerTask>,
    /// Tasks waiting to be inserted into the `tasks` map, because their outbound substream is still opening.
    pending_tasks: Vec<(Option<(u32, bool)>, PendingHandlerTask<Box<dyn Any + Send>>)>,
}

impl<const N: usize, S: Store<N>> KamilataHandler<N, S> {
    pub(crate) fn new(our_peer_id: PeerId, remote_peer_id: PeerId, db: Arc<Db<N, S>>, config: Arc<KamilataConfig>) -> Self {
        KamilataHandler {
            our_peer_id,
            remote_peer_id,
            db,
            config,
            rt_handle: tokio::runtime::Handle::current(),
            task_counter: Counter::new(3),
            tasks: HashMap::new(),
            pending_tasks: Vec::new(),
        }
    }
}

impl<const N: usize, S: Store<N>> ConnectionHandler for KamilataHandler<N, S> {
    type FromBehaviour = BehaviorToHandlerEvent<N, S>;
    type ToBehaviour = HandlerToBehaviorEvent;
    type Error = ioError;
    type InboundProtocol = Either<ArcConfig, DeniedUpgrade>;
    type OutboundProtocol = ArcConfig;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = (Option<(u32, bool)>, PendingHandlerTask<Box<dyn Any + Send>>);

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        SubstreamProtocol::new(ArcConfig::from(&self.config), ()).map_upgrade(Either::Left)
    }

    // Events are sent by the Behaviour which we need to obey to.
    fn on_behaviour_event(&mut self, event: Self::FromBehaviour) {
        trace!("{} Received event: {event:?}", self.our_peer_id);
        match event {
            BehaviorToHandlerEvent::Request { request, sender } => {
                let pending_task = pending_request::<N>(request, sender, self.our_peer_id, self.remote_peer_id);
                self.pending_tasks.push((None, pending_task));
            },
            BehaviorToHandlerEvent::SearchRequest { query, routes_sender, result_sender, over_notifier  } => {
                let pending_task = pending_search_req::<N, S>(query, routes_sender, result_sender, over_notifier, self.our_peer_id, self.remote_peer_id);
                self.pending_tasks.push((None, pending_task));
            },
            BehaviorToHandlerEvent::LeechFilters => {
                if self.tasks.contains_key(&2) || self.pending_tasks.iter().any(|(_, pending_task)| pending_task.name == "leech_filters") {
                    trace!("{} Already leeching filters from {}", self.our_peer_id, self.remote_peer_id);
                    return;
                }
                let pending_task = pending_leech_filters(Arc::clone(&self.db), self.our_peer_id, self.remote_peer_id);
                self.pending_tasks.push((Some((2, true)), pending_task))
            },
            BehaviorToHandlerEvent::StopLeeching => {
                self.pending_tasks.retain(|(_, pending_task)| pending_task.name != "leech_filters");
                if self.tasks.remove(&2).is_some() {
                    let behaviour_controller = self.db.behaviour_controller().clone();
                    let remote_peer_id = self.remote_peer_id;
                    tokio::spawn(async move {
                        behaviour_controller.emit_event(KamilataEvent::SeederRemoved { peer_id: remote_peer_id }).await;
                    });
                }
            },
            BehaviorToHandlerEvent::StopSeeding => {
                if self.tasks.remove(&1).is_some() {
                    let behaviour_controller = self.db.behaviour_controller().clone();
                    let remote_peer_id = self.remote_peer_id;
                    tokio::spawn(async move {
                        behaviour_controller.emit_event(KamilataEvent::LeecherRemoved { peer_id: remote_peer_id }).await;
                    });
                }
            },
        };
    }

    fn connection_keep_alive(&self) -> KeepAlive {
        KeepAlive::Yes
    }

    #[warn(implied_bounds_entailment)]
    fn on_connection_event(
        &mut self,
        event: ConnectionEvent<
            Self::InboundProtocol,
            Self::OutboundProtocol,
            Self::InboundOpenInfo,
            Self::OutboundOpenInfo,
        >,
    ) {
        match event {
            // When we receive an inbound channel, a task is immediately created to handle the channel.
            ConnectionEvent::FullyNegotiatedInbound(i) => {
                let substream = match i.protocol {
                    futures::future::Either::Left(s) => s,
                    futures::future::Either::Right(_void) => return,
                };
        
                // TODO: prevent DoS
                let fut = handle_request(substream, Arc::clone(&self.db), self.our_peer_id, self.remote_peer_id).boxed();
                self.tasks.insert(self.task_counter.next(), HandlerTask { fut, name: "handle_request" });
            },
            // Once an outbound is fully negotiated, the pending task which requested the establishment of the channel is now ready to be executed.
            ConnectionEvent::FullyNegotiatedOutbound(i) => {
                let (tid, pending_task) = i.info;
                let fut = (pending_task.fut)(i.protocol, pending_task.params);
                let (tid, replace) = tid.unwrap_or_else(|| (self.task_counter.next(), true));
                if self.tasks.contains_key(&tid) && !replace {
                    return;
                }
                if let Some(old_task) = self.tasks.insert(tid, HandlerTask { fut, name: pending_task.name }) {
                    warn!("{} Replaced {} task with {} task at tid={tid}", self.our_peer_id, old_task.name, pending_task.name)
                }        
            },
            ConnectionEvent::DialUpgradeError(i) => {
                let (_tid, pending_task) = i.info;
                let error = i.error;
                warn!("{} Failed to establish outbound channel with {}: {error:?}. A {} task has been discarded.", self.our_peer_id, self.remote_peer_id, pending_task.name);
            },
            ConnectionEvent::ListenUpgradeError(_) | ConnectionEvent::AddressChange(_) | ConnectionEvent::LocalProtocolsChange(_) | ConnectionEvent::RemoteProtocolsChange(_) => (),
        }
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<
        ConnectionHandlerEvent<
            Self::OutboundProtocol,
            Self::OutboundOpenInfo,
            Self::ToBehaviour,
            Self::Error,
        >,
    > {
        // It seems this method gets called in a context where the tokio runtime does not exist.
        // We import that runtime so that we can rely on it.
        let _rt_enter_guard = self.rt_handle.enter();

        // Poll tasks
        for tid in self.tasks.keys().copied().collect::<Vec<u32>>() {
            let task = self.tasks.get_mut(&tid).unwrap();

            match task.fut.poll_unpin(cx) {
                Poll::Ready(output) => {
                    trace!("{} Task {} completed (tid={tid})", self.our_peer_id, task.name);
                    self.tasks.remove(&tid);
                    
                    match tid {
                        1 => {
                            let behaviour_controller = self.db.behaviour_controller().clone();
                            let remote_peer_id = self.remote_peer_id;
                            tokio::spawn(async move {
                                behaviour_controller.emit_event(KamilataEvent::LeecherRemoved { peer_id: remote_peer_id }).await;
                            });
                        }
                        2 => {
                            let behaviour_controller = self.db.behaviour_controller().clone();
                            let remote_peer_id = self.remote_peer_id;
                            tokio::spawn(async move {
                                behaviour_controller.emit_event(KamilataEvent::SeederRemoved { peer_id: remote_peer_id }).await;
                            });
                        }
                        _ => ()
                    }

                    for output in output.into_vec() {
                        match output {
                            HandlerTaskOutput::SetTask { tid, mut task } => {
                                match self.tasks.get(&tid) {
                                    Some(old_task) => warn!("{} Replacing {} task with {} task at tid={tid}", self.our_peer_id, old_task.name, task.name),
                                    None => trace!("{} Inserting {} task at tid={tid}", self.our_peer_id, task.name)                                    ,
                                }
                                if let Poll::Ready(output) = task.fut.poll_unpin(cx) {
                                    if !matches!(output, HandlerTaskOutput::None) {
                                        error!("{} Task {} completed immediately after being inserted (tid={tid})", self.our_peer_id, task.name);
                                    }
                                } else {
                                    self.tasks.insert(tid, task);
                                }
                            },
                            HandlerTaskOutput::NewPendingTask { tid, pending_task } => {
                                trace!("{} New pending task: {}", self.our_peer_id, pending_task.name);
                                self.pending_tasks.push((tid, pending_task));
                            },
                            HandlerTaskOutput::Disconnect(disconnect_packet) => {
                                debug!("{} Disconnected peer {}", self.our_peer_id, self.remote_peer_id);
                                // TODO: send packet
                                return Poll::Ready(ConnectionHandlerEvent::Close(
                                    ioError::new(std::io::ErrorKind::Other, disconnect_packet.reason), // TODO error handling
                                ));
                            },
                            HandlerTaskOutput::None | HandlerTaskOutput::Many(_) => unreachable!(),
                        }
                    }
                }
                Poll::Pending => ()
            }
        }   

        if let Some((tid, pending_task)) = self.pending_tasks.pop() {
            return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                protocol: SubstreamProtocol::new(ArcConfig::from(&self.config), (tid, pending_task)),
            })
        }

        // It seems we don't have to care about waking up the handler because libp2p does it when inject methods are called.
        // A link to documentation would be appreciated.
        Poll::Pending
    }
}
