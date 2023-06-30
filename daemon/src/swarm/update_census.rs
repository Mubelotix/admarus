use crate::prelude::*;

pub async fn update_census_task(node: NodeController, keypair: Keypair, config: Arc<Args>) {
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
            .map(|rec| rec.addr.to_string())
            .filter(|a| !external_addrs.contains(a))
            .collect::<Vec<_>>();
        external_addrs.extend(new_addrs);
        
        if external_addrs.is_empty() {
            sleep(Duration::from_secs(30*60)).await;
            continue;
        }

        let record = Record {
            peer_id: keypair.public().to_peer_id().to_string(),
            addrs: external_addrs.clone(),
        };

        match submit_census_record(&config.census_rpc, record, keypair.clone()).await {
            Ok(()) => trace!("Submitted census record"),
            Err(e) => error!("Failed to publish census record: {:?}", e),
        }

        sleep(Duration::from_secs(30*60)).await;
    }
}
