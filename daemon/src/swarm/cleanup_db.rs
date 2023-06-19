use crate::prelude::*;

/// Removes entries older than 1 week in the known peers database.
pub async fn cleanup_db_task(controller: NodeController) {
    loop {
        let mut known_peers = controller.sw.known_peers.write().await;
        let previous_len = known_peers.len();
        let now = now();
        known_peers.retain(|_, info| {
            match info.last_updated() {
                Some(last_updated) => now - last_updated < 7*86400,
                None => false,
            }
        });
        let new_len = known_peers.len();
        drop(known_peers);
        if new_len != previous_len {
            debug!("Removed {} peers from known peers database (outdated data)", previous_len - new_len);
        }

        sleep(Duration::from_secs(60*60)).await;
    }
}
