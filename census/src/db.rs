use crate::prelude::*;

lazy_static::lazy_static! {
    pub static ref DB: Db = Db::open();
}

#[derive(Default)]
pub struct Db {
    ips: RwLock<HashSet<String>>,
    pub records: RwLock<Vec<DbRecord>>,
    pub drain_history: RwLock<Vec<u64>>,
    pub stats: RwLock<GetStatsResp>,
}

impl Db {
    pub fn open() -> Db {
        let drain_history_data = std::fs::read_to_string("drain_history.txt").unwrap_or_default();
        let drain_history = drain_history_data
            .split('\n')
            .filter(|line| !line.is_empty())
            .map(|line| line.parse::<u64>().expect("Invalid value in drain_history.txt"))
            .collect::<Vec<_>>();

        Db {
            drain_history: RwLock::new(drain_history),
            ..Default::default()
        }
    }

    pub async fn insert_record(&self, record: Record, ip: String) {
        let mut ips = self.ips.write().await;
        let ip_tainted = !ips.insert(ip.clone());
        drop(ips);

        #[cfg(feature = "ip_filter")]
        if ip_tainted {
            eprintln!("Ignoring spam from {ip}");
            return;
        }

        let mut records = self.records.write().await;

        let last_ts = records.iter().rfind(|r| r.r.peer_id == record.peer_id).map(|r| r.ts);
        if let Some(last_ts) = last_ts {
            if last_ts > now_ts() - 3*60 {
                eprintln!("Ignoring record from {} (already have a record from {} seconds ago)", record.peer_id, now_ts() - last_ts);
                return;
            }
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
        use tokio::signal::{ctrl_c, unix::{SignalKind, signal}};
        
        let crtl_c_fut = ctrl_c();
        let mut sig_term_receiver = signal(SignalKind::terminate()).expect("Failed to listen for SIGTERM");
        let sig_term_fut = sig_term_receiver.recv();
        select(Box::pin(crtl_c_fut), Box::pin(sig_term_fut)).await;
        self.drain(None).await;
        println!("Database ready to shut down");
    }
    
    async fn save_drain_history(&self) {
        let drain_history = self.drain_history.read().await;
        let drain_history_data = drain_history.iter().map(|ts| ts.to_string()).collect::<Vec<_>>().join("\n");
        drop(drain_history);
        match tokio::fs::write("drain_history.txt", drain_history_data).await {
            Ok(()) => (),
            Err(e) => eprintln!("Failed to write drain history: {e}"),
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
        self.save_drain_history().await;
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
