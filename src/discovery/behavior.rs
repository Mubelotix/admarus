use super::*;

pub enum Event {

}

pub struct Behaviour {
    config: Arc<Config>,
    db: Arc<Db>,
    events_to_dispatch: Vec<(PeerId, HandlerInEvent)>,
}

#[derive(Debug, Clone)]
pub struct PeerListQuery {
    peer_id: PeerId,
    protocol_version: Option<String>,
    agent_version: Option<String>,
    protocols: Option<Vec<String>>,
    metadata: Option<Vec<u8>>,
    max_results: Option<usize>,
}

impl PeerListQuery {
    pub fn new(peer_id: PeerId) -> PeerListQuery {
        PeerListQuery {
            peer_id,
            protocol_version: None,
            agent_version: None,
            protocols: None,
            metadata: None,
            max_results: None,
        }
    }

    pub fn with_protocol_version(mut self, protocol_version: String) -> Self {
        self.protocol_version = Some(protocol_version);
        self
    }

    pub fn with_agent_version(mut self, agent_version: String) -> Self {
        self.agent_version = Some(agent_version);
        self
    }

    pub fn with_protocols(mut self, protocols: Vec<String>) -> Self {
        self.protocols = Some(protocols);
        self
    }

    pub fn with_protocol(mut self, protocol: String) -> Self {
        self.protocols = match self.protocols {
            Some(mut protocols) => {
                protocols.push(protocol);
                Some(protocols)
            },
            None => Some(vec![protocol]),
        };
        self
    }

    pub fn with_metadata(mut self, metadata: Vec<u8>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

impl Behaviour {
    pub async fn query(&mut self, query: PeerListQuery) -> Result<HashMap<PeerId, Info>, IoError> {
        let (sender, receiver) = oneshot_channel();
        self.events_to_dispatch.push((
            query.peer_id,
            HandlerInEvent::Request {
                request: Request::GetPeers {
                    protocol_version: query.protocol_version,
                    agent_version: query.agent_version,
                    protocols: query.protocols,
                    metadata: query.metadata,
                    max_results: query.max_results.unwrap_or(self.config.max_results),
                },
                replier: sender
            }
        ));
        let result = receiver.await.map_err(|_| IoError::new(std::io::ErrorKind::BrokenPipe, "Couldn't receive response"))?;
        match result {
            Ok(Response::Peers(peers)) => {
                let mut final_peers = HashMap::new();
                for (peer_id, info) in peers {
                    if let Ok(peer_id) = peer_id.parse() {
                        final_peers.insert(peer_id, info);
                    }
                }
                Ok(final_peers)
            },
            Ok(_) => Err(IoError::new(std::io::ErrorKind::InvalidData, "Unexpected response")),
            Err(e) => Err(e)
        }
    }
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = Handler;
    type OutEvent = Event;

    fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
        match event {
            FromSwarm::ConnectionEstablished(info) => {
                let db = Arc::clone(&self.db);
                tokio::spawn(async move {
                    db.insert_peer(info.peer_id).await;
                });
            },
            FromSwarm::ConnectionClosed(info) => {
                if info.remaining_established == 0 {
                    let db = Arc::clone(&self.db);
                    tokio::spawn(async move {
                        db.remove_peer(&info.peer_id).await;
                    });
                }
            },
            _ => (),
        }
    }

    fn on_connection_handler_event(&mut self, _peer_id: PeerId, _connection_id: ConnectionId, event: THandlerOutEvent<Self>) {
        match event {}
    }

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        remote_peer_id: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        Ok(Handler::new(remote_peer_id, Arc::clone(&self.config), Arc::clone(&self.db)))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        remote_peer_id: PeerId,
        _addr: &Multiaddr,
        _role_override: libp2p::core::Endpoint,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        Ok(Handler::new(remote_peer_id, Arc::clone(&self.config), Arc::clone(&self.db)))
    }

    fn poll(&mut self, cx: &mut Context<'_>, params: &mut impl PollParameters) -> Poll<ToSwarm<Self::OutEvent, THandlerInEvent<Self>>> {
        if let Some((peer_id, event)) = self.events_to_dispatch.pop() {
            return Poll::Ready(ToSwarm::NotifyHandler { peer_id, handler: NotifyHandler::Any, event });
        }

        Poll::Pending
    }
}
