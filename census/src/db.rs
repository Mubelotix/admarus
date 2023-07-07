use crate::prelude::*;

lazy_static::lazy_static! {
    pub static ref DB: Db = Db {
        ips: RwLock::new(HashSet::new()),
        records: RwLock::new(Vec::new()),
        drain_history: RwLock::new(Vec::new()),
    };
}

pub struct Db {
    ips: RwLock<HashSet<String>>,
    records: RwLock<Vec<Record>>,
    drain_history: RwLock<Vec<Instant>>,
}

impl Db {
    pub async fn insert_record(&self, record: Record, ip: String) {
        let mut ips = self.ips.write().await;
        let ip_tainted = ips.insert(ip.clone());
        drop(ips);

        let mut records = self.records.write().await;
        let previous_len = records.len();
        records.retain(|r| r.peer_id != record.peer_id);
        let is_new = previous_len == records.len();

        #[cfg(feature = "ip_filter")]
        if is_new && ip_tainted {
            eprintln!("Ignoring spam from {ip}");
            return;
        }

        if records.len() >= 55000 {
            eprintln!("Database is full, ignoring record from {ip}");
            return;
        }
        
        records.push(record);
    }

    pub async fn draw_peers(&self, count: usize, exclude: &[String]) -> Vec<(String, Vec<String>)> {
        self.records.read().await
            .iter()
            .filter(|r| !exclude.contains(&r.peer_id))
            .choose_multiple(&mut rand::thread_rng(), count)
            .into_iter()
            .map(|r| (r.peer_id.clone(), r.addrs.clone()))
            .collect::<Vec<_>>()
    }

    pub async fn get_stats(&self) -> GetStatsResp {
        todo!()
    }

    pub async fn shutdowner(&self) {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                self.drain(None).await;
                println!("Database ready to shut down");
            },
            Err(err) => eprintln!("Unable to listen for shutdown signal: {err}"),
        }
    }

    async fn drain(&self, to_drain: Option<usize>) {
        let mut records = self.records.write().await;
        let to_drain = to_drain.unwrap_or(records.len());
        let drained = records.drain(..to_drain).collect::<Vec<_>>();
        if drained.is_empty() {
            return;
        }
        drop(records);

        let mut drain_history = self.drain_history.write().await;
        let drain_index = drain_history.len();
        drain_history.push(Instant::now());
        drop(drain_history);
        
        let drained_json = match serde_json::to_string(&drained) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("Failed to serialize records: {e}");
                return;
            }
        };
        let filename = format!("data_{drain_index}.json");
        let r = tokio::fs::write(&filename, drained_json).await;
        if let Err(e) = r {
            eprintln!("Failed to write records to {filename}: {e}");
        }
    }

    pub async fn run(&self) {
        let mut i = 0;
        loop {
            i += 1;
            sleep(Duration::from_secs(60)).await;
            
            if i % 30 == 0 {
                self.ips.write().await.clear()
            }

            let records = self.records.read().await;
            if records.len() <= 1000 {
                continue;
            }
            let to_drain = records.len() - 1000;
            drop(records);

            self.drain(Some(to_drain)).await;
        }
    }
}
