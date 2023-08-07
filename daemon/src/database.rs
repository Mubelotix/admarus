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
    async fn index_get(&self, key: String) -> Result<HashMap<LocalCid, f32>, DbError> {
        let (sender, receiver) = oneshot_channel();
        self.sender.send(DbCommand::IndexGet{key, sender}).await.map_err(|_| DbError::CommandChannelUnavailable)?;
        Ok(receiver.await.map_err(|_| DbError::UnresponsiveDatabase)??)
    }

    async fn index_put(&self, key: String, index: HashMap<LocalCid, f32>) -> Result<(), DbError> {
        let (sender, receiver) = oneshot_channel();
        self.sender.send(DbCommand::IndexWrite{key, value: index, sender}).await.map_err(|_| DbError::CommandChannelUnavailable)?;
        Ok(receiver.await.map_err(|_| DbError::UnresponsiveDatabase)??)
    }
}

#[derive(Clone)]
/// A [DbController] that is restricted to index-related commands
pub struct DbIndexController(DbController);
impl DbIndexController {
    pub async fn get(&self, key: String) -> Result<HashMap<LocalCid, f32>, DbError> { self.0.index_get(key).await }
    pub async fn put(&self, key: String, index: HashMap<LocalCid, f32>) -> Result<(), DbError> { self.0.index_put(key, index).await }
}
impl From<DbController> for DbIndexController { fn from(controller: DbController) -> Self { DbIndexController(controller) } }

enum DbCommand {
    IndexGet { key: String, sender: OneshotSender<Result<HashMap<LocalCid, f32>, HeedError>> },
    IndexWrite { key: String, value: HashMap<LocalCid, f32>, sender: OneshotSender<Result<(), HeedError>> },
}

impl std::fmt::Debug for DbCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbCommand::IndexGet { key, .. } => f.debug_struct("IndexGet").field("key", key).finish_non_exhaustive(),
            DbCommand::IndexWrite { key, value, .. } => f.debug_struct("IndexWrite").field("key", key).field("index", &format!("{:?} entries", value.len())).finish_non_exhaustive(),
        }
    }
}

fn index_get(key: &str, env: &Env, index: &HeedDatabase<Str, ByteSlice>) -> Result<HashMap<LocalCid, f32>, HeedError> {
    let rotxn = env.read_txn()?;
    let data = index.get(&rotxn, key)?.unwrap_or_default();
    let mut value: HashMap<LocalCid, f32> = HashMap::new();
    for chunk in data.chunks_exact(8) {
        let lcid: u32 = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let score: f32 = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
        value.insert(LocalCid(lcid), score);
    }
    Ok(value)
}

fn index_put(key: &str, value: HashMap<LocalCid, f32>, env: &Env, index: &HeedDatabase<Str, ByteSlice>) -> Result<(), HeedError> {
    let mut wtxn = env.write_txn()?;
    let mut data = Vec::with_capacity(value.len() * 8);
    for (lcid, score) in value {
        data.extend_from_slice(&lcid.0.to_le_bytes());
        data.extend_from_slice(&score.to_le_bytes());
    }
    index.put(&mut wtxn, key, &data)?;
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
            DbCommand::IndexWrite { key, value, sender } => {
                let result = index_put(&key, value, &env, &index);
                let r = sender.send(result);
                if let Err(e) = r { error!("Failed to send index database write result: {e:?}") }
            },
        }
    }
}

pub fn open_database(database_path: &str) -> DbController {
    trace!("Opening database at {database_path}");

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
