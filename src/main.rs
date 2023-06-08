mod result;
mod store;
mod prelude;
mod crawl;
mod documents;

#[tokio::main]
async fn main() {
    println!("Hello, world!");
    let pinned = crawl::list_pinned().await;
    println!("{:?}", pinned);
    let pinned_files = crawl::explore_all(pinned).await;
    println!("{:#?}", pinned_files);
    let documents = crawl::collect_documents(pinned_files).await;
    println!("{} documents", documents.len());
}
