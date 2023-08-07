use crate::prelude::*;
use heed::{Database as HeedDatabase, Error as HeedError, Env, EnvOpenOptions, types::*};
use futures::executor::block_on;

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
    async fn index_get(&self, key: String) -> Result<Vec<(LocalCid, f32)>, DbError> {
        let (sender, receiver) = oneshot_channel();
        self.sender.send(DbCommand::IndexGet{key, sender}).await.map_err(|_| DbError::CommandChannelUnavailable)?;
        Ok(receiver.await.map_err(|_| DbError::UnresponsiveDatabase)??)
    }

    async fn index_put_batch(&self, items: Vec<(String, HashMap<LocalCid, f32>)>) -> Result<(), DbError> {
        let (sender, receiver) = oneshot_channel();
        self.sender.send(DbCommand::IndexPutBatch{items, sender}).await.map_err(|_| DbError::CommandChannelUnavailable)?;
        Ok(receiver.await.map_err(|_| DbError::UnresponsiveDatabase)??)
    }

    async fn index_put(&self, key: String, value: HashMap<LocalCid, f32>) -> Result<(), DbError> {
        self.index_put_batch(vec![(key, value)]).await
    }
}

#[derive(Clone)]
/// A [DbController] that is restricted to index-related commands
pub struct DbIndexController(DbController);
impl DbIndexController {
    pub async fn get(&self, key: String) -> Result<Vec<(LocalCid, f32)>, DbError> { self.0.index_get(key).await }
    pub async fn put(&self, key: String, value: HashMap<LocalCid, f32>) -> Result<(), DbError> { self.0.index_put(key, value).await }
    pub async fn put_batch(&self, items: Vec<(String, HashMap<LocalCid, f32>)>) -> Result<(), DbError> { self.0.index_put_batch(items).await }
}
impl From<DbController> for DbIndexController { fn from(controller: DbController) -> Self { DbIndexController(controller) } }

enum DbCommand {
    IndexGet { key: String, sender: OneshotSender<Result<Vec<(LocalCid, f32)>, HeedError>> },
    IndexPutBatch { items: Vec<(String, HashMap<LocalCid, f32>)>, sender: OneshotSender<Result<(), HeedError>> },
}

impl std::fmt::Debug for DbCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbCommand::IndexGet { key, .. } => f.debug_struct("IndexGet").field("key", key).finish_non_exhaustive(),
            DbCommand::IndexPutBatch { items, .. } => f.debug_struct("IndexWriteAll").field("index", &format!("{:?} entries", items.len())).finish_non_exhaustive(),
        }
    }
}

fn index_get(key: &str, env: &Env, index: &HeedDatabase<Str, ByteSlice>) -> Result<Vec<(LocalCid, f32)>, HeedError> {
    let rotxn = env.read_txn()?;
    let data = index.get(&rotxn, key)?.unwrap_or_default();
    let mut value = Vec::with_capacity(data.len() / 8);
    for chunk in data.chunks_exact(8) {
        let lcid: u32 = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let score: f32 = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        value.push((LocalCid(lcid), score));
    }
    Ok(value)
}

fn index_put_batch(items: &[(String, HashMap<LocalCid, f32>)], env: &Env, index: &HeedDatabase<Str, ByteSlice>) -> Result<(), HeedError> {
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

fn run_database(env: Env, index: HeedDatabase<Str, ByteSlice>, mut receiver: Receiver<DbCommand>) {
    loop {
        // Receive command
        let Some(command) = block_on(receiver.recv()) else {
            warn!("Database command channel closed, stopping database thread");
            break;
        };
        
        // TODO: ensure db is empty

        // Execute command
        match command {
            DbCommand::IndexGet { key, sender } => {
                let result = index_get(&key, &env, &index);
                let r = sender.send(result);
                if let Err(e) = r { error!("Failed to send index database read result: {e:?}") }
            },
            DbCommand::IndexPutBatch { items, sender } => {
                let result = index_put_batch(&items, &env, &index);
                let r = sender.send(result);
                if let Err(e) = r { error!("Failed to send index database write result: {e:?}") }
            }
        }
    }
}

pub fn open_database(database_path: &str) -> DbController {
    trace!("Opening database at {database_path}");

    let _ = std::fs::remove_dir(database_path); // FIXME: remove this line
    std::fs::create_dir_all(database_path).expect("Failed to create directories to database");
    let env = EnvOpenOptions::new()
        .map_size(25_000 * 4096) // ~100MB
        .max_dbs(15)
        .max_readers(200) // TODO check those default values
        .open(database_path)
        .expect("Failed to open database");

    let mut wtxn = env.write_txn().expect("Failed to open write transaction for index database creation");
    let index = env.create_database(&mut wtxn, Some("index")).expect("Failed to create index database");
    wtxn.commit().expect("Failed to commit write transaction for index database creation");

    let (sender, receiver) = channel(200);
    
    std::thread::spawn(move || run_database(env, index, receiver));

    DbController {
        sender,
    }
}
