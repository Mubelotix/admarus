mod result;
mod index;
mod prelude;
mod crawl;
mod documents;
mod api;
mod kamilata;
mod clap;

use crate::prelude::*;


#[tokio::main]
async fn main() {
    let args = Args::parse();

    env_logger::init();

    let index = DocumentIndex::<125000>::new(args.ipfs_rpc);
    
    let search_park = Arc::new(SearchPark::new());

    let kamilata = KamilataNode::init(args.kam_addr, index.clone()).await;
    let kamilata = Arc::new(kamilata.run());
    if let Some(bootstrap_addr) = args.kam_bootstrap {
        kamilata.dial(bootstrap_addr.parse().unwrap()).await;
        sleep(Duration::from_secs(2)).await;
        kamilata.leech_from_all().await; // FIXME: remove this
    }

    let f1 = serve_api(&args.api_addr, index.clone(), search_park, kamilata);
    let f2 = index.run();
    tokio::join!(f1, f2);
}
