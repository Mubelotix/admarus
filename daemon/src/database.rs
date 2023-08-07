use crate::prelude::*;
use heed::{Database as HeedDatabase, Env, EnvOpenOptions, types::*};
use futures::executor::block_on;

#[derive(Clone)]
pub struct DatabaseController {
    sender: Sender<DatabaseCommand>,
}

enum DatabaseCommand {

}

impl std::fmt::Debug for DatabaseCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            _ => write!(f, "DatabaseCommand"),
        }
    }
}

fn run_database(env: Env, index: HeedDatabase<Str, CowSlice<u32>>, mut receiver: Receiver<DatabaseCommand>) {
    loop {
        // Receive command
        let command = match block_on(receiver.recv()) {
            Some(command) => {
                trace!("Received database command: {:?}", command);
                command
            },
            None => {
                warn!("Database command channel closed, stopping database thread");
                break;
            },
        };


    }
}

pub fn open_database(database_path: &str) -> DatabaseController {
    trace!("Opening database at {database_path}");

    let env = EnvOpenOptions::new()
        .map_size(100_000_000)
        .max_dbs(15)
        .max_readers(200) // TODO check those default values
        .open(database_path)
        .expect("Failed to open database");

    let mut wtxn = env.write_txn().expect("Failed to open write transaction for index database creation");
    let index = env.create_database(&mut wtxn, Some("index")).expect("Failed to create index database");
    wtxn.commit().expect("Failed to commit write transaction for index database creation");

    let (sender, receiver) = channel(200);
    
    std::thread::spawn(move || run_database(env, index, receiver));

    DatabaseController {
        sender,
    }
}
