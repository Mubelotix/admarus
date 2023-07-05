#![allow(implied_bounds_entailment)]
#![allow(clippy::module_inception)]

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
mod discovery;
mod query;
mod dns_pins;

use crate::prelude::*;


#[tokio::main]
async fn main() {
    let config = Arc::new(Args::parse());
    if config.api_addr != "127.0.0.1:3030" {
        warn!("The webui doesn't currently support custom api addresses, so you probably don't want to change this.")
    }

    env_logger::init();

    let index = DocumentIndex::<125000>::new(Arc::clone(&config));
    
    let search_park = Arc::new(SearchPark::new());

    let (node, keypair) = Node::init(Arc::clone(&config), index.clone()).await;
    let node = node.run();

    let f1 = serve_api(&config.api_addr, index.clone(), search_park, node.clone());
    let f2 = index.run();
    let f3 = maintain_swarm_task(node.clone(), Arc::clone(&config));
    let f4 = cleanup_db_task(node.clone());
    let f5 = update_census_task(node.clone(), keypair.clone(), Arc::clone(&config));
    let f6 = manage_dns_pins(Arc::clone(&config));
    tokio::join!(f1, f2, f3, f4, f5, f6);
}
