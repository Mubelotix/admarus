use super::*;

#[derive(Debug, Clone)]
pub enum Event {

}

pub struct Behaviour {
    config: Arc<Config>,
    db: Arc<Db>,
    events_to_dispatch: Vec<(PeerId, HandlerInEvent)>,
}

impl Behaviour {
    pub fn new() -> Behaviour {
        Behaviour::new_with_config(Config::default())
    }

    pub fn new_with_config(config: Config) -> Behaviour {
        let config = Arc::new(config);
        Behaviour {
            config: Arc::clone(&config),
            db: Arc::new(Db::new(config)),
            events_to_dispatch: Vec::new(),
        }
    }

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

    pub async fn set_info(&mut self, peer_id: PeerId, info: libp2p_identify::Info) {
        self.db.set_info(&peer_id, Info {
            protocol_version: info.protocol_version,
            agent_version: info.agent_version,
            listen_addrs: info.listen_addrs,
            protocols: info.protocols,
            observed_addr: Some(info.observed_addr),
            metadata: Vec::new(),
        }).await;
    }

    pub async fn set_visibility_to_peer(&mut self, peer_id: PeerId, visible: bool) -> Result<(), IoError> {
        let (sender, receiver) = oneshot_channel();
        self.events_to_dispatch.push((
            peer_id,
            HandlerInEvent::Request {
                request: Request::SetVisibility(visible),
                replier: sender
            }
        ));
        let rep = receiver.await.map_err(|_| IoError::new(std::io::ErrorKind::BrokenPipe, "Couldn't receive response"))??;
        if matches!(rep, Response::Ok) {
            Ok(())
        } else {
            Err(IoError::new(std::io::ErrorKind::InvalidData, "Unexpected response"))
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

    fn poll(&mut self, _cx: &mut Context<'_>, _params: &mut impl PollParameters) -> Poll<ToSwarm<Self::OutEvent, THandlerInEvent<Self>>> {
        if let Some((peer_id, event)) = self.events_to_dispatch.pop() {
            return Poll::Ready(ToSwarm::NotifyHandler { peer_id, handler: NotifyHandler::Any, event });
        }

        Poll::Pending
    }
}
