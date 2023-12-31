use crate::prelude::*;

pub async fn update_census_task(node: NodeController, index: DocumentIndex, keypair: Keypair, config: Arc<Args>) {
    if !config.census_enabled {
        return;
    }
    let external_addrs = match config.external_addrs.clone() {
        Some(external_addrs) => external_addrs,
        None => {
            warn!("No external address specified. Your node might not be able to advertise itself to others.");
            Vec::new()
        },
    };

    loop {
        let mut external_addrs = external_addrs.clone();
        let new_addrs = node
            .external_addresses().await
            .into_iter()
            .map(|a| a.to_string())
            .filter(|a| !external_addrs.contains(a))
            .collect::<Vec<_>>();
        external_addrs.extend(new_addrs);
        
        if external_addrs.is_empty() {
            warn!("Failed to advertise ourselves to census due to lack of known external addresses");
            sleep(Duration::from_secs(30*60)).await;
            continue;
        }

        let mut folders = index.folders().await.into_iter().map(|(cid, count)| (cid, count as u64)).collect::<Vec<_>>();
        if folders.len() > 500 {
            folders.sort_by(|(_, a), (_, b)| b.cmp(a));
            folders.truncate(500);
        }
        let record = Record {
            peer_id: keypair.public().to_peer_id().to_string(),
            addrs: external_addrs.clone(),
            folders,
        };

        match submit_census_record(&config.census_rpc, record, keypair.clone()).await {
            Ok(()) => trace!("Submitted census record"),
            Err(e) => error!("Failed to publish census record: {:?}", e),
        }

        sleep(Duration::from_secs(30*60)).await;
    }
}
