use crate::prelude::*;

/// Asks a central server for a list of peers.
pub async fn get_peers_from_census(node: NodeController, census_rpc: &str) {
    let census_peers = match get_census_peers(census_rpc).await {
        Ok(peers) => peers,
        Err(e) => {
            error!("Failed to get peers from census: {e:?}");
            return;
        }
    };

    let now = now();
    let mut known_peers = node.sw.known_peers.write().await;
    let previous_len = known_peers.len();
    for (peer_id, addrs) in census_peers {
        let known_peer = known_peers.entry(peer_id).or_default();
        for addr in addrs {
            if !known_peer.addrs.contains(&addr) {
                known_peer.addrs.push(addr);
            }
        }
        known_peer.last_returned_by_census = Some(now);
    }
    let new_len = known_peers.len();
    drop(known_peers);

    if new_len != previous_len {
        debug!("Got {} new peers from census", new_len - previous_len);
    }
}

/// Some of our IPFS peers might run Admarus.
/// We try to infer their potential Admarus listen addresses from their IPFS addresses.
pub async fn get_peers_from_ipfs(controller: NodeController, config: Arc<Args>) {
    let ipfs_peers = match get_ipfs_peers(&config.ipfs_rpc).await {
        Ok(peers) => peers,
        Err(e) => {
            error!("Failed to get peers from IPFS: {e:?}");
            return;
        }
    };

    let now = now();
    let mut known_peers = controller.sw.known_peers.write().await;
    let previous_len = known_peers.len();
    for (peer_id, ipfs_addr) in ipfs_peers {
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
        debug!("Got {} new peers from IPFS", new_len - previous_len);
    }
}

/// Asks our peers for a list of their peers.
pub async fn get_peers_from_others(controller: NodeController, _config: Arc<Args>) {
    let connected_peers = controller.sw.connected_peers.read().await.keys().cloned().collect::<Vec<_>>();

    let queries = connected_peers.into_iter().map(|p| {
        PeerListQuery::new(p) // TODO more params
    });

    let mut tasks: Vec<BoxFuture<_>> = Vec::new();
    for q in queries {
        tasks.push(Box::pin(controller.query_peers(q)))
    }

    join_all(tasks).await;
}

pub async fn get_peers(controller: NodeController, config: Arc<Args>) {
    let mut tasks: Vec<BoxFuture<_>> = Vec::new();
    if let Some(census_rpc) = &config.census_rpc {
        let census_task = get_peers_from_census(controller.clone(), census_rpc);
        tasks.push(Box::pin(census_task));
    }

    let ipfs_task = get_peers_from_ipfs(controller.clone(), Arc::clone(&config));
    tasks.push(Box::pin(ipfs_task));

    let discovery_task = get_peers_from_others(controller, Arc::clone(&config));
    tasks.push(Box::pin(discovery_task));

    join_all(tasks).await;
}
