use crate::prelude::*;

pub async fn update_census_task(node: NodeController, keypair: Keypair, config: Arc<Args>) {
    let census_rpc = match config.census_rpc {
        Some(ref census_rpc) => census_rpc,
        None => return,
    };
    let external_addrs = match config.external_addrs.clone() {
        Some(external_addrs) => external_addrs,
        None => {
            warn!("No external address specified. Your node might not be able to advertise itself to others.");
            Vec::new()
        },
    };

    loop {
        let mut external_addrs = external_addrs.clone();
        external_addrs.extend(node
            .external_addresses().await
            .into_iter()
            .map(|rec| rec.addr.to_string()));
        
        if external_addrs.is_empty() {
            sleep(Duration::from_secs(30*60)).await;
            continue;
        }

        let record = Record {
            peer_id: keypair.public().to_peer_id().to_string(),
            addrs: external_addrs.clone(),
        };

        match submit_census_record(census_rpc, record, keypair.clone()).await {
            Ok(()) => trace!("Submitted census record"),
            Err(e) => error!("Failed to publish census record: {:?}", e),
        }

        sleep(Duration::from_secs(30*60)).await;
    }
}
