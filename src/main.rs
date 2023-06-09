mod result;
mod index;
mod prelude;
mod crawl;
mod documents;
mod api;

use crate::prelude::*;


#[tokio::main]
async fn main() {
    println!("Hello, world!");
    let index = DocumentIndex::<125000>::new();

    let f1 = serve_api(index.clone());
    let f2 = index.run();
    tokio::join!(f1, f2);
}
