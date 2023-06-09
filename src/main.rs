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
    let pinned = list_pinned().await;
    println!("{:?}", pinned);
    let pinned_files = explore_all(pinned).await;
    println!("{:#?}", pinned_files);
    let documents = collect_documents(pinned_files).await;
    println!("{} documents", documents.len());
    let index = DocumentIndex::<125000>::new();
    index.add_documents(documents).await;
    index.update_filter().await;
    println!("{:.04}%", index.get_filter().await.load()*100.0);

    serve_api(index).await;
}
