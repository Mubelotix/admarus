mod result;
mod index;
mod prelude;
mod crawl;
mod documents;

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
    let mut index = DocumentIndex::<125000>::new();
    index.add_documents(documents);
    println!("{:#?} {} words", index.index, index.index.len());
}
