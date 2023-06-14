use crate::prelude::*;

/// Some of our ipfs peers might run Admarus.
/// We try to connect randomly on the default Admarus port (4002).
pub async fn follow_ipfs_task(controller: KamilataController, config: Arc<Args>) {
    loop {
        let peers = match get_ipfs_peers(&config.ipfs_rpc).await {
            Ok(peers) => peers,
            Err(e) => {
                error!("Failed to bootstrap from ipfs peers: {e:?}");
                return;
            }
        };
    
        let now = Instant::now();
        let mut known_peers = controller.sw.known_peers.write().await;
        let previous_len = known_peers.len();
        for (peer_id, ipfs_addr) in peers {
            let addr_components = ipfs_addr.iter().collect::<Vec<_>>();
            let mut admarus_addr = Multiaddr::empty();
            match addr_components.first() {
                Some(Protocol::Ip4(ip)) => {
                    admarus_addr.push(Protocol::Ip4(*ip));
                    admarus_addr.push(Protocol::Tcp(4002));
                }
                Some(Protocol::Ip6(ip)) => {
                    admarus_addr.push(Protocol::Ip6(*ip));
                    admarus_addr.push(Protocol::Tcp(4002));
                }
                _ => continue,
            }
            let known_peer = known_peers.entry(peer_id).or_default();
            if !known_peer.addrs.contains(&admarus_addr) {
                known_peer.addrs.push(admarus_addr);
            }
            known_peer.last_seen_ipfs = Some(now);
        }
        let new_len = known_peers.len();
        drop(known_peers);
        if new_len != previous_len {
            debug!("Added {} new peers from ipfs", new_len - previous_len);
        }

        sleep(Duration::from_secs(5*60)).await;
    }
}
