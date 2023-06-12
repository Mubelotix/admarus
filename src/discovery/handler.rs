use super::*;

#[derive(Debug)]
pub enum HandlerInEvent {
    Request {
        request: Request,
        replier: RequestReplier,
    },
}

#[derive(Debug)]
pub enum HandlerOutEvent {}

#[derive(Debug)]
pub enum HandlerError {}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DiscoveryHandlerError")
    }
}

impl std::error::Error for HandlerError {}

pub struct Handler {
    remote_peer_id: PeerId,
    config: Arc<Config>,
    db: Arc<Db>,

    server_tasks: Vec<BoxFuture<'static, Result<(), IoError>>>,
    client_tasks: Vec<BoxFuture<'static, Result<Response, IoError>>>,
    pending_requests: Vec<(Request, OneshotSender<Result<Response, IoError>>)>,
}

impl Handler {
    pub fn new(remote_peer_id: PeerId, config: Arc<Config>, db: Arc<Db>) -> Handler {
        Handler {
            remote_peer_id,
            config,
            db,
            server_tasks: Vec::new(),
            client_tasks: Vec::new(),
            pending_requests: Vec::new(),
        }
    }
}


impl ConnectionHandler for Handler {
    type InEvent = HandlerInEvent;
    type OutEvent = HandlerOutEvent;
    type Error = HandlerError;
    type InboundProtocol = ArcConfig;
    type OutboundProtocol = ArcConfig;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = (Request, RequestReplier);

    fn listen_protocol(&self) -> SubstreamProtocol<ArcConfig, ()> {
        SubstreamProtocol::new((&self.config).into(), ())
    }

    fn connection_keep_alive(&self) -> KeepAlive {
        KeepAlive::Yes
    }

    fn on_behaviour_event(&mut self, event: HandlerInEvent) {
        match event {
            HandlerInEvent::Request { request, replier } => {
                self.pending_requests.push((request, replier))
            },
        }
    }

    fn on_connection_event(&mut self, event: ConnectionEvent<ArcConfig, ArcConfig, (), (Request, RequestReplier)>) {
        match event {
            ConnectionEvent::FullyNegotiatedInbound(info) => {
                let stream = info.protocol;
                let server_task = Box::pin(server_task(self.remote_peer_id, stream, Arc::clone(&self.db)));
                self.server_tasks.push(server_task)
            },
            ConnectionEvent::FullyNegotiatedOutbound(info) => {
                let stream = info.protocol;
                let (request, replier) = info.info;
                let client_task = Box::pin(client_task(request, stream, Arc::clone(&self.db)));
                self.client_tasks.push(client_task)
            },
            ConnectionEvent::DialUpgradeError(e) => {
                let e = e.error;
                error!("DialUpgradeError: {e:?}");
            },
            ConnectionEvent::ListenUpgradeError(e) => {
                let e = e.error;
                error!("ListenUpgradeError: {e:?}");
            },
            ConnectionEvent::AddressChange(_) => (),
        }
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<ConnectionHandlerEvent<ArcConfig, (Request, RequestReplier), HandlerOutEvent, HandlerError>> {
        if let Some(pending_info) = self.pending_requests.pop() {
            return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                protocol: SubstreamProtocol::new((&self.config).into(), pending_info),
            });
        }

        // Run server task
        if let Some(server_task) = self.server_tasks.first_mut() {
            match server_task.as_mut().poll(cx) {
                Poll::Ready(result) => {
                    self.server_tasks.remove(0);
                    debug!("Server task finished");
                },
                Poll::Pending => (),
            }
        }

        // Run client task
        if let Some(client_task) = self.client_tasks.first_mut() {
            match client_task.as_mut().poll(cx) {
                Poll::Ready(result) => {
                    self.client_tasks.remove(0);
                    debug!("Client task finished");
                },
                Poll::Pending => (),
            }
        }

        Poll::Pending
    }
}
