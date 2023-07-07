use std::collections::HashMap;

use crate::prelude::*;

lazy_static::lazy_static! {
    pub static ref DB: Db = Db {
        ips: RwLock::new(HashSet::new()),
        records: RwLock::new(Vec::new()),
        drain_history: RwLock::new(Vec::new()),
        stats: RwLock::new(GetStatsResp::default()),
    };
}

pub struct Db {
    ips: RwLock<HashSet<String>>,
    records: RwLock<Vec<DbRecord>>,
    drain_history: RwLock<Vec<u64>>,
    stats: RwLock<GetStatsResp>,
}

impl Db {
    pub async fn insert_record(&self, record: Record, ip: String) {
        let mut ips = self.ips.write().await;
        let ip_tainted = ips.insert(ip.clone());
        drop(ips);

        let mut records = self.records.write().await;
        let previous_len = records.len();
        records.retain(|r| r.r.peer_id != record.peer_id);
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
        
        records.push(DbRecord {
            r: record,
            ts: now_ts(),
        });
    }

    pub async fn draw_peers(&self, count: usize, exclude: &[String]) -> Vec<(String, Vec<String>)> {
        self.records.read().await
            .iter()
            .filter(|r| !exclude.contains(&r.r.peer_id))
            .choose_multiple(&mut rand::thread_rng(), count)
            .into_iter()
            .map(|r| (r.r.peer_id.clone(), r.r.addrs.clone()))
            .collect::<Vec<_>>()
    }

    pub async fn get_stats(&self) -> GetStatsResp {
        self.stats.read().await.clone()
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
        drain_history.push(now_ts());
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

    #[allow(clippy::too_many_arguments)]
    pub async fn compute_stats(&self) {
        let mut peers_24h = HashSet::new();
        let mut folders_24h = HashMap::new();
        let mut peer_documents_24h = HashMap::new();

        let mut prev_peers_24h = HashSet::new();
        let mut prev_folders_24h = HashMap::new();
        let mut prev_peer_documents_24h = HashMap::new();

        let mut peers_1h = HashSet::new();
        let mut folders_1h = HashMap::new();
        let mut peer_documents_1h = HashMap::new();

        let mut prev_peers_1h = HashSet::new();
        let mut prev_folders_1h = HashMap::new();
        let mut prev_peer_documents_1h = HashMap::new();

        fn count_record(record: Record, peers: &mut HashSet<String>, folders: &mut HashMap<String, u64>, peer_documents: &mut HashMap<String, u64>) {
            peers.insert(record.peer_id.clone());
            let mut file_count = 0;
            for (cid, count) in record.folders {
                let already_counted = folders.entry(cid).or_default();
                if *already_counted < count {
                    *already_counted = count;
                }
                file_count += count;
            }
            peer_documents.insert(record.peer_id, file_count);
        }
        
        fn count_records(
            records: Vec<DbRecord>, now: u64,
            peers_24h: &mut HashSet<String>, peers_1h: &mut HashSet<String>, prev_peers_24h: &mut HashSet<String>, prev_peers_1h: &mut HashSet<String>,
            folders_24h: &mut HashMap<String, u64>, folders_1h: &mut HashMap<String, u64>, prev_folders_24h: &mut HashMap<String, u64>, prev_folders_1h: &mut HashMap<String, u64>,
            peer_documents_24h: &mut HashMap<String, u64>, peer_documents_1h: &mut HashMap<String, u64>, prev_peer_documents_24h: &mut HashMap<String, u64>, prev_peer_documents_1h: &mut HashMap<String, u64>,
        ) {
            for DbRecord { r, ts } in records {
                if ts > now - 86400 {
                    if ts > now - 3600 {
                        count_record(r.clone(), peers_1h, folders_1h, peer_documents_1h)
                    } else if ts > now - 2*3600 {
                        count_record(r.clone(), prev_peers_1h, prev_folders_1h, prev_peer_documents_1h)
                    }
                    count_record(r, peers_24h, folders_24h, peer_documents_24h)
                } else if ts > now - 2*86400 {
                    count_record(r, prev_peers_24h, prev_folders_24h, prev_peer_documents_24h)
                }
            }
        }

        fn into_stats(peers: HashSet<String>, folders: HashMap<String, u64>, peer_documents: HashMap<String, u64>) -> NetworkStats {
            NetworkStats {
                peers: peers.len() as u64,
                documents: peer_documents.values().sum::<u64>(),
                different_documents: folders.values().sum::<u64>(),
                median_documents_per_peer: {
                    let mut peer_documents_1h = peer_documents.into_values().collect::<Vec<_>>();
                    peer_documents_1h.sort_unstable();
                    match peer_documents_1h.len() {
                        0 => 0,
                        len => peer_documents_1h[len / 2],
                    }
                },
            }
        }

        // Start by using the data that's in Db before it expires
        let now = now_ts();
        let first_drain_24h = self.drain_history.read().await.iter().position(|ts| *ts > now - 86400);
        let first_drain = first_drain_24h.map(|i| i - 1);
        let drain_len = self.drain_history.read().await.len();
        let current_records = self.records.read().await.clone();
        count_records(
            current_records, now, &mut peers_24h, &mut peers_1h, &mut prev_peers_24h, &mut prev_peers_1h,
            &mut folders_24h, &mut folders_1h, &mut prev_folders_24h, &mut prev_folders_1h,
            &mut peer_documents_24h, &mut peer_documents_1h, &mut prev_peer_documents_24h, &mut prev_peer_documents_1h
        );

        // Finish by reading the data that's on disk
        if let Some(first_drain) = first_drain {
            for i in first_drain..drain_len {
                let records = match tokio::fs::read_to_string(format!("data_{i}.json")).await {
                    Ok(json) => match serde_json::from_str::<Vec<DbRecord>>(&json) {
                        Ok(records) => records,
                        Err(e) => {
                            eprintln!("Failed to deserialize records from data_{i}.json: {e}");
                            continue;
                        }
                    },
                    Err(e) => {
                        eprintln!("Failed to read data_{i}.json: {e}");
                        continue;
                    }
                };
                count_records(
                    records, now, &mut peers_24h, &mut peers_1h, &mut prev_peers_24h, &mut prev_peers_1h,
                    &mut folders_24h, &mut folders_1h, &mut prev_folders_24h, &mut prev_folders_1h,
                    &mut peer_documents_24h, &mut peer_documents_1h, &mut prev_peer_documents_24h, &mut prev_peer_documents_1h
                );
            }
        }

        // Collect the stats
        let stats = GetStatsResp {
            stats_1h: into_stats(peers_1h, folders_1h, peer_documents_1h),
            prev_stats_1h: into_stats(prev_peers_1h, prev_folders_1h, prev_peer_documents_1h),
            stats_24h: into_stats(peers_24h, folders_24h, peer_documents_24h),
            prev_stats_24h: into_stats(prev_peers_24h, prev_folders_24h, prev_peer_documents_24h),
        };

        // Update the stats
        *self.stats.write().await = stats;
    }

    pub async fn update_stats_task(&self) {
        loop {
            sleep(Duration::from_secs(5*60)).await;
            self.compute_stats().await;
        }
    }

    pub async fn drain_task(&self) {
        let mut i = 0;
        loop {
            i += 1;
            sleep(Duration::from_secs(60)).await;
            
            if i % 20 == 0 {
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
