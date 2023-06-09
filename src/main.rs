mod result;
mod index;
mod prelude;
mod crawl;
mod documents;

use std::convert::Infallible;
use crate::prelude::*;
use warp::{Filter, http::{Response, StatusCode}};

#[derive(Deserialize, Serialize)]
struct SearchUrlQuery {
    q: String,
}

async fn search<const N: usize>((query, index): (SearchUrlQuery, DocumentIndex<N>)) -> Result<impl warp::Reply, Infallible> {
    let words: Vec<_> = query.q.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 3).map(|w| w.to_string()).collect();
    let words_len = words.len();
    let results = index.search(words, words_len).await;
    Ok(Response::builder().header("Content-Type", "application/json").body(serde_json::to_string(&results).unwrap()).unwrap())
}

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

    let hello_world = warp::path::end().map(|| "Hello, World at root!");

    let search = warp::get()
        .and(warp::path("search"))
        .and(warp::query::<SearchUrlQuery>())
        .map(move |q: SearchUrlQuery| (q, index.clone()))
        .and_then(search);

    let routes = warp::get().and(
        hello_world
            .or(search)
    );

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
