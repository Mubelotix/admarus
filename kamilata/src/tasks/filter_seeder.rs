//! This module contains the task responsible for broadcasting local filters to remote peers.

use super::*;

// TODO: When rejecting a peer, we should send a message to the peer explaining why we rejected it

pub(crate) async fn seed_filters<const N: usize, S: Store<N>>(
    mut stream: KamInStreamSink<Stream>,
    mut req: GetFiltersPacket,
    db: Arc<Db<N, S>>,
    our_peer_id: PeerId,
    remote_peer_id: PeerId
) -> HandlerTaskOutput {
    trace!("{our_peer_id} Seed filters task executing");

    // Checks if we should allow this peer to leech
    if let Some(approve_leecher) = &db.get_config().approve_leecher {
        if !approve_leecher(remote_peer_id).await {
            warn!("{our_peer_id} {remote_peer_id} wasn't approved to leech");
            return HandlerTaskOutput::None;
        }
    }
    
    // Claims a spot as a leecher for the remote peer
    if let Err(TooManyLeechers{}) = db.add_leecher(remote_peer_id).await {
        warn!("{our_peer_id} Too many leechers, can't seed to {remote_peer_id}");
        return HandlerTaskOutput::None;
    }

    // Determine an interval
    let config = db.get_config();
    req.filter_count = req.filter_count.clamp(0, config.filter_count as u8); // unsafe cast
    let interval = match config.get_filters_interval.intersection(&req.interval) {
        Some(interval) => interval.target() as u64,
        None => {
            warn!("{our_peer_id} Couldn't agree on interval with {remote_peer_id} (ours: {:?}, theirs: {:?})", config.get_filters_interval, req.interval);
            return HandlerTaskOutput::None;
        }
    };

    // Send an event
    db.behaviour_controller().emit_event(KamilataEvent::LeecherAdded {
        peer_id: remote_peer_id,
        filter_count: req.filter_count as usize,
        interval_ms: interval as usize,
    }).await;

    let mut peers_to_ignore = req.blocked_peers.to_libp2p_peer_ids();
    peers_to_ignore.push(remote_peer_id);

    loop {
        let our_filters = db.get_filters_bytes(&peers_to_ignore).await; // FIXME: filter count isn't respected
        stream.start_send_unpin(ResponsePacket::UpdateFilters(UpdateFiltersPacket { filters: our_filters })).unwrap();
        if stream.flush().await.is_err() {
            warn!("{our_peer_id} Couldn't send filters to {remote_peer_id}");
            return HandlerTaskOutput::None;
        } 
        trace!("{our_peer_id} Sent filters to {remote_peer_id}");

        sleep(Duration::from_millis(interval)).await;
    }
}
