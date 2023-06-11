use super::*;

#[derive(Debug)]
pub enum HandlerInEvent {}

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
    db: Arc<Db>,

    server_task: Option<BoxFuture<'static, Result<(), IoError>>>,
    client_task: Option<BoxFuture<'static, Result<Response, IoError>>>,
}

impl Handler {
    pub fn new(remote_peer_id: PeerId, db: Arc<Db>) -> Handler {
        Handler {
            remote_peer_id,
            db,
            server_task: None,
            client_task: None,
        }
    }
}


impl ConnectionHandler for Handler {
    type InEvent = HandlerInEvent;
    type OutEvent = HandlerOutEvent;
    type Error = HandlerError;
    type InboundProtocol = Discovery;
    type OutboundProtocol = Discovery;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Discovery, ()> {
        let discovery = Discovery { protocols: Arc::new(vec![]) };
        SubstreamProtocol::new(discovery, ())
    }

    fn connection_keep_alive(&self) -> KeepAlive {
        KeepAlive::Yes
    }

    fn on_behaviour_event(&mut self, event: HandlerInEvent) {
        match event {

        }
    }

    fn on_connection_event(&mut self, event: ConnectionEvent<Discovery, Discovery, (), ()>) {
        match event {
            ConnectionEvent::FullyNegotiatedInbound(info) => {
                let stream = info.protocol;
                let server_task = Box::pin(server_task(self.remote_peer_id, stream, Arc::clone(&self.db)));
                self.server_task = Some(server_task);
            },
            ConnectionEvent::FullyNegotiatedOutbound(info) => {
                let stream = info.protocol;
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

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<ConnectionHandlerEvent<Discovery, (), HandlerOutEvent, HandlerError>> {
        // Run server task
        if let Some(server_task) = self.server_task.as_mut() {
            match server_task.as_mut().poll(cx) {
                Poll::Ready(result) => {
                    self.server_task = None;
                    debug!("Server task finished");
                },
                Poll::Pending => (),
            }
        }

        // Run client task
        if let Some(client_task) = self.client_task.as_mut() {
            match client_task.as_mut().poll(cx) {
                Poll::Ready(result) => {
                    self.client_task = None;
                    debug!("Client task finished");
                },
                Poll::Pending => (),
            }
        }

        Poll::Pending
    }
}
