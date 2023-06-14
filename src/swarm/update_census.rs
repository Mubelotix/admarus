use crate::prelude::*;

pub async fn update_census_task(node: NodeController, census_rpc: Option<&str>, keypair: Keypair) {
    let census_rpc = match census_rpc {
        Some(census_rpc) => census_rpc,
        None => return,
    };

    loop {
        let addrs = node
            .external_addresses().await
            .into_iter()
            .map(|rec| rec.addr.to_string())
            .collect();

        let record = Record {
            peer_id: keypair.public().to_peer_id().to_string(),
            addrs,
        };

        match submit_census_record(census_rpc, record, keypair.clone()).await {
            Ok(()) => trace!("Submitted census record"),
            Err(e) => error!("Failed to publish census record: {:?}", e),
        }

        sleep(Duration::from_secs(30*60)).await;
    }
}
