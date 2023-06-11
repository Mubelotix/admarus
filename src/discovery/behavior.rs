use super::*;

pub enum Event {

}

pub struct Behaviour {
    db: Arc<Db>,
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = Handler;
    type OutEvent = Event;

    fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
        match event {
            FromSwarm::ConnectionEstablished(_) => todo!(),
            FromSwarm::ConnectionClosed(_) => todo!(),
            FromSwarm::AddressChange(_) => todo!(),
            FromSwarm::DialFailure(_) => todo!(),
            FromSwarm::ListenFailure(_) => todo!(),
            FromSwarm::NewListener(_) => todo!(),
            FromSwarm::NewListenAddr(_) => todo!(),
            FromSwarm::ExpiredListenAddr(_) => todo!(),
            FromSwarm::ListenerError(_) => todo!(),
            FromSwarm::ListenerClosed(_) => todo!(),
            FromSwarm::NewExternalAddr(_) => todo!(),
            FromSwarm::ExpiredExternalAddr(_) => todo!(),
        }
    }

    fn on_connection_handler_event(&mut self, peer_id: PeerId, connection_id: ConnectionId, event: THandlerOutEvent<Self>) {
        match event {

        }
    }

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        remote_peer_id: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        Ok(Handler::new(remote_peer_id, Arc::clone(&self.db)))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        remote_peer_id: PeerId,
        _addr: &Multiaddr,
        _role_override: libp2p::core::Endpoint,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        Ok(Handler::new(remote_peer_id, Arc::clone(&self.db)))
    }

    fn poll(&mut self, cx: &mut Context<'_>, params: &mut impl PollParameters) -> Poll<ToSwarm<Self::OutEvent, THandlerInEvent<Self>>> {
        todo!()
    }
}
