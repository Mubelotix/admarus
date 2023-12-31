use crate::prelude::*;
use heed::{Database as HeedDatabase, Error as HeedError, Env, EnvOpenOptions, types::*, zerocopy::U32};
use futures::executor::block_on;
use heed::byteorder::LE;
use bimap::BiHashMap;

type LEU32 = U32<LE>;

#[derive(Debug)]
pub enum DbError {
    CommandChannelUnavailable,
    UnresponsiveDatabase,
    Heed(HeedError),
}

impl From<HeedError> for DbError {
    fn from(e: HeedError) -> Self {
        DbError::Heed(e)
    }
}

#[derive(Clone)]
pub struct DbController {
    sender: Sender<DbCommand>,
}

impl DbController {
    async fn index_get(&self, keys: Vec<String>) -> Result<Vec<(String, Vec<(LocalCid, f32)>)>, DbError> {
        let (sender, receiver) = oneshot_channel();
        self.sender.send(DbCommand::IndexGet{keys, sender}).await.map_err(|_| DbError::CommandChannelUnavailable)?;
        Ok(receiver.await.map_err(|_| DbError::UnresponsiveDatabase)??)
    }

    async fn index_put(&self, items: Vec<(String, HashMap<LocalCid, f32>)>) -> Result<(), DbError> {
        let (sender, receiver) = oneshot_channel();
        self.sender.send(DbCommand::IndexPut{items, sender}).await.map_err(|_| DbError::CommandChannelUnavailable)?;
        Ok(receiver.await.map_err(|_| DbError::UnresponsiveDatabase)??)
    }

    async fn put_cids(&self, items: Vec<(LocalCid, String)>) -> Result<(), DbError> {
        let (sender, receiver) = oneshot_channel();
        self.sender.send(DbCommand::PutCids{items, sender}).await.map_err(|_| DbError::CommandChannelUnavailable)?;
        Ok(receiver.await.map_err(|_| DbError::UnresponsiveDatabase)??)
    }

    pub async fn compute_filter(&self) -> Result<Filter<FILTER_SIZE>, DbError> {
        let (sender, receiver) = oneshot_channel();
        self.sender.send(DbCommand::ComputeFilter{sender}).await.map_err(|_| DbError::CommandChannelUnavailable)?;
        Ok(receiver.await.map_err(|_| DbError::UnresponsiveDatabase)??)
    }
}

#[derive(Clone)]
/// A [DbController] that is restricted to index-related commands
pub struct DbIndexController(DbController);
impl DbIndexController {
    pub async fn get(&self, keys: Vec<String>) -> Result<Vec<(String, Vec<(LocalCid, f32)>)>, DbError> { self.0.index_get(keys).await }
    pub async fn put(&self, items: Vec<(String, HashMap<LocalCid, f32>)>) -> Result<(), DbError> { self.0.index_put(items).await }
    pub async fn put_cids(&self, items: Vec<(LocalCid, String)>) -> Result<(), DbError> { self.0.put_cids(items).await }
    pub async fn compute_filter(&self) -> Result<Filter<FILTER_SIZE>, DbError> { self.0.compute_filter().await }
}
impl From<DbController> for DbIndexController { fn from(controller: DbController) -> Self { DbIndexController(controller) } }

enum DbCommand {
    IndexGet { keys: Vec<String>, sender: OneshotSender<Result<Vec<(String, Vec<(LocalCid, f32)>)>, HeedError>> },
    IndexPut { items: Vec<(String, HashMap<LocalCid, f32>)>, sender: OneshotSender<Result<(), HeedError>> },
    PutCids { items: Vec<(LocalCid, String)>, sender: OneshotSender<Result<(), HeedError>> },
    ComputeFilter { sender: OneshotSender<Result<Filter<FILTER_SIZE>, HeedError>> },
}

impl std::fmt::Debug for DbCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbCommand::IndexGet { keys, .. } => f.debug_struct("IndexGet").field("keys", &format!("{:?} entries", keys.len())).finish_non_exhaustive(),
            DbCommand::IndexPut { items, .. } => f.debug_struct("IndexPut").field("index", &format!("{:?} entries", items.len())).finish_non_exhaustive(),
            DbCommand::PutCids { items, .. } => f.debug_struct("PutCids").field("cids", &format!("{:?} entries", items.len())).finish_non_exhaustive(),
            DbCommand::ComputeFilter { .. } => f.debug_struct("ComputeFilter").finish_non_exhaustive(),
        }
    }
}

fn index_get(keys: Vec<String>, env: &Env, index: &HeedDatabase<Str, ByteSlice>) -> Result<Vec<(String, Vec<(LocalCid, f32)>)>, HeedError> {
    let rotxn = env.read_txn()?;
    let mut items = Vec::with_capacity(keys.len());
    for key in keys {
        let data = index.get(&rotxn, &key)?.unwrap_or_default();
        let mut value = Vec::with_capacity(data.len() / 8);
        for chunk in data.chunks_exact(8) {
            let lcid: u32 = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let score: f32 = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            value.push((LocalCid(lcid), score));
        }
        items.push((key, value));
    }
    Ok(items)
}

fn index_put(items: &[(String, HashMap<LocalCid, f32>)], env: &Env, index: &HeedDatabase<Str, ByteSlice>) -> Result<(), HeedError> {
    let mut wtxn = env.write_txn()?;
    for (key, value) in items {
        let mut data = Vec::with_capacity(value.len() * 8);
        for (lcid, score) in value {
            data.extend_from_slice(&lcid.0.to_le_bytes());
            data.extend_from_slice(&score.to_le_bytes());
        }
        index.put(&mut wtxn, key, &data)?;
    }
    wtxn.commit()?;
    Ok(())
}

fn put_cids(items: Vec<(LocalCid, String)>, env: &Env, cids: &HeedDatabase<OwnedType<LEU32>, Str>) -> Result<(), HeedError> {
    let mut wtxn = env.write_txn()?;
    for (lcid, cid) in items {
        cids.put(&mut wtxn, &LEU32::new(lcid.0), &cid)?;
    }
    wtxn.commit()?;
    Ok(())
}

fn compute_filter(env: &Env, index: &HeedDatabase<Str, ByteSlice>) -> Result<Filter<FILTER_SIZE>, HeedError> {
    let mut filter = Filter::new();

    let rotxn = env.read_txn().expect("Failed to open read transaction to iterate over words");
    let index = index.iter(&rotxn).expect("Failed to iterate over words").filter_map(|c| c.ok());
    for (word, _) in index {
        filter.add_word::<DocumentIndex>(word);
    }

    Ok(filter)
}

fn run_database(env: Env, index: HeedDatabase<Str, ByteSlice>, cids: HeedDatabase<OwnedType<LEU32>, Str>, mut receiver: Receiver<DbCommand>) {
    loop {
        // Receive command
        let Some(command) = block_on(receiver.recv()) else {
            warn!("Database command channel closed, stopping database thread");
            break;
        };

        // Execute command
        match command {
            DbCommand::IndexGet { keys, sender } => {
                let result = index_get(keys, &env, &index);
                let r = sender.send(result);
                if let Err(e) = r { error!("Failed to send index database read result: {e:?}") }
            },
            DbCommand::IndexPut { items, sender } => {
                let result = index_put(&items, &env, &index);
                let r = sender.send(result);
                if let Err(e) = r { error!("Failed to send index database write result: {e:?}") }
            },
            DbCommand::PutCids { items, sender } => {
                let result = put_cids(items, &env, &cids);
                let r = sender.send(result);
                if let Err(e) = r { error!("Failed to send cids database write result: {e:?}") }
            },
            DbCommand::ComputeFilter { sender } => {
                let result = compute_filter(&env, &index);
                let r = sender.send(result);
                if let Err(e) = r { error!("Failed to send filter computation result: {e:?}") }
            },
        }
    }
}

pub fn open_database(config: Arc<Args>) -> (DbController, u32, BiHashMap<LocalCid, String>) {
    trace!("Opening database at {}", config.database_path);

    // Open env
    std::fs::create_dir_all(&config.database_path).expect("Failed to create directories to database");
    let env = EnvOpenOptions::new()
        .map_size(config.database_map_size)
        .max_dbs(15)
        .max_readers(config.database_max_readers)
        .open(&config.database_path)
        .expect("Failed to open database");

    // Create databases
    let mut wtxn = env.write_txn().expect("Failed to open write transaction for database creation");
    let index = env.create_database(&mut wtxn, Some("index")).expect("Failed to create index database");
    let cid_db: HeedDatabase<OwnedType<LEU32>, Str> = env.create_database(&mut wtxn, Some("cids")).expect("Failed to create cids database");
    wtxn.commit().expect("Failed to commit write transaction for database creation");

    // Retrieve all cids
    let start = Instant::now();
    let rotxn = env.read_txn().expect("Failed to open read transaction for cid restoration");
    let db_cids = cid_db.iter(&rotxn).expect("Failed to iterate over cids database").filter_map(|c| c.ok());
    let mut cids = BiHashMap::new();
    let mut max = 0;
    for (lcid, cid) in db_cids {
        let lcid = lcid.get();
        if max < lcid {
            max = lcid;
        }
        cids.insert(LocalCid(lcid), cid.to_owned());
    }
    drop(rotxn);
    if !cids.is_empty() {
        debug!("{} documents retrieved from disk in {}ms", cids.len(), start.elapsed().as_millis());
    }

    let (sender, receiver) = channel(200);    
    std::thread::spawn(move || run_database(env, index, cid_db, receiver));

    (DbController{sender}, max+100_000 /* TODO: refine value */, cids)
}
