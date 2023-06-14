use crate::prelude::*;

/// Ensures the swarm is healthy.
pub async fn maintain_swarm_task(controller: NodeController, config: Arc<Args>) {
    let sw = Arc::clone(&controller.sw);

    let mut dial_attemps: HashMap<PeerId, Instant> = HashMap::new();

    loop {
        // Unselect all first-class peers that are not seeding
        // Disconnect all transient peers that are staying too long
        for (peer_id, info) in sw.connected_peers.write().await.iter_mut() {
            match info.class() {
                PeerClass::First => if !info.seeding && info.connected_since.elapsed() > Duration::from_secs(60) {
                    info.selected = false;
                    // TODO visility in discovery
                },
                PeerClass::Transient => if info.connected_since.elapsed() > Duration::from_secs(60) {
                    debug!("Disconnecting transient peer {peer_id}");
                    controller.disconnect(peer_id).await;
                },
                _ => (),
            }
        }

        // Sweep dial_attemps
        dial_attemps.retain(|_,time| time.elapsed() < Duration::from_secs(3600));
        let currently_dialing = dial_attemps.values().filter(|t| t.elapsed() < Duration::from_secs(30)).count();

        // Looking for more peers
        let (fcp_count, _scp_count, _tp_count) = sw.class_counts().await;
        let missing_fcp = config.first_class.saturating_sub(fcp_count).saturating_sub(currently_dialing);
        if missing_fcp == 0 { continue }
        trace!("Not enough first-class peers, looking for {missing_fcp} more ({} targeted - {fcp_count} have - {currently_dialing} dialing)", config.first_class);

        {
            let known_peers = sw.known_peers.read().await;
            let connected_peers = sw.connected_peers.read().await;

            let mut candidates = known_peers
                .iter()
                .filter(|(peer_id, _)| 
                    !connected_peers.get(peer_id).map(|i| i.selected).unwrap_or(false)
                    && (!dial_attemps.contains_key(peer_id) || connected_peers.contains_key(peer_id))
                )
                .collect::<Vec<_>>();
            candidates.sort_by(|(aid, a), (bid, b)| {
                let mut ordering = b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal);
                if ordering == Ordering::Equal {
                    ordering = connected_peers.contains_key(bid).cmp(&connected_peers.contains_key(aid));
                }
                ordering
            });
            candidates.truncate(missing_fcp);
            drop(connected_peers);
            
            let mut connected_peers = sw.connected_peers.write().await;
            for (peer_id, info) in candidates {
                if let Some(connected_info) = connected_peers.get_mut(peer_id) {
                    debug!("Selecting first-class peer {peer_id}");
                    connected_info.selected = true;
                    controller.leech_from(*peer_id).await;
                } else {
                    debug!("Dialing new peer {peer_id} at {:?}", info.addrs);
                    controller.dial_with_peer_id(*peer_id, info.addrs.clone()).await;
                    dial_attemps.insert(*peer_id, Instant::now());
                }
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}
