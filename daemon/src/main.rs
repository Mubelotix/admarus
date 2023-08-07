#![allow(implied_bounds_entailment)]
#![allow(clippy::module_inception)]
#![recursion_limit = "256"]

mod result;
mod index;
mod prelude;
mod rpc_ipfs;
mod rpc_census;
mod documents;
mod api;
mod node;
mod clap;
mod swarm;
mod query;
mod dns_pins;

#[cfg(any(feature = "database-lmdb", feature = "database-mdbx"))]
mod database;

use crate::prelude::*;


#[tokio::main]
async fn main() {
    env_logger::init();
    
    let mut config = Args::parse();
    if let Some(addrs) = &mut config.external_addrs {
        for addr in addrs {
            if addr.parse::<Multiaddr>().is_err() {
                error!("Invalid external address: {addr}");
            }
        }
    }

    let config = Arc::new(config);
    if config.api_addr != "127.0.0.1:5002" {
        warn!("The webui doesn't currently support custom api addresses, so you probably don't want to change this.")
    }

    let index = match (&config.database_path, cfg!(any(feature = "database-lmdb", feature = "database-mdbx"))) {
        (Some(database_path), true) => {
            let db = open_database(&database_path);
            DocumentIndex::<125000>::new(Arc::clone(&config), Some(db))
        },
        (Some(_), false) => panic!("This program was not compiled with database support. Please recompile with the `database-lmdb` or `database-mdbx` feature enabled. See https://github.com/Mubelotix/admarus/wiki/Installation"),
        (None, _) => DocumentIndex::<125000>::new(Arc::clone(&config), None),
    };

    let (node, keypair) = Node::init(Arc::clone(&config), index.clone()).await;
    let node = node.run();
    
    let search_park = Arc::new(SearchPark::new());

    let f1 = serve_api(Arc::clone(&config), index.clone(), search_park, node.clone());
    let f2 = update_census_task(node.clone(), index.clone(), keypair.clone(), Arc::clone(&config));
    let f3 = maintain_swarm_task(node.clone(), Arc::clone(&config));
    let f4 = cleanup_db_task(node.clone());
    let f5 = manage_dns_pins(Arc::clone(&config));
    let f6 = index.run();
    tokio::join!(f1, f2, f3, f4, f5, f6);
}
