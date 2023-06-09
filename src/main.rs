mod result;
mod index;
mod prelude;
mod crawl;
mod documents;
mod api;
mod kamilata;

use crate::prelude::*;


#[tokio::main]
async fn main() {
    env_logger::init();
    let index = DocumentIndex::<125000>::new();
    let kamilata = KamilataNode::init(index.clone()).await;
    let kamilata = Arc::new(kamilata.run());
    let search_park = Arc::new(SearchPark::new());

    let f1 = serve_api(index.clone(), search_park, kamilata);
    let f2 = index.run();
    tokio::join!(f1, f2);
}
