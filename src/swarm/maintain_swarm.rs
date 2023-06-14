use crate::prelude::*;

/// Ensures the swarm is healthy.
pub async fn maintain_swarm_task(controller: NodeController, config: Arc<Args>) {
    let sw = Arc::clone(&controller.sw);
    let mut last_get_peers: Option<Instant> = None;

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

        // Looking for more peers
        sw.sweep_dial_attempts().await;
        let currently_dialing = sw.currently_dialing().await;
        let (fcp_count, _scp_count, _tp_count) = sw.class_counts().await;
        let missing_fcp = config.first_class.saturating_sub(fcp_count).saturating_sub(currently_dialing);
        if missing_fcp == 0 { continue }
        trace!("Not enough first-class peers, looking for {missing_fcp} more ({} targeted - {fcp_count} have - {currently_dialing} dialing)", config.first_class);
        let candidates = sw.get_peers_to_dial(missing_fcp).await;

        // Fetch more peers if we don't have enough
        if candidates.len() < missing_fcp && last_get_peers.map(|i| i.elapsed() > Duration::from_secs(60)).unwrap_or(true) {
            trace!("Not enough candidates ({}). Getting peers", candidates.len());
            last_get_peers = Some(Instant::now());
            let controller2 = controller.clone();
            let config2 = Arc::clone(&config);
            tokio::spawn(async move {
                get_peers(controller2, config2).await
            });
        }

        {            
            let mut known_peers = sw.known_peers.write().await;
            let mut dial_attempts = sw.dial_attemps.write().await;
            let mut connected_peers = sw.connected_peers.write().await;
            for (peer_id, info) in candidates {
                if let Some(connected_info) = connected_peers.get_mut(&peer_id) {
                    debug!("Selecting first-class peer {peer_id}");
                    connected_info.selected = true;
                    controller.leech_from(peer_id).await;
                } else {
                    debug!("Dialing new peer {peer_id} at {:?}", info.addrs);
                    known_peers.entry(peer_id).or_default().failed_dials += 1; // We count as failed but it will be canceled if it succeeds
                    controller.dial_with_peer_id(peer_id, info.addrs).await;
                    dial_attempts.insert(peer_id, Instant::now());
                }
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}
