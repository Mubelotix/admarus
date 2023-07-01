use crate::prelude::*;
use libp2p::PeerId;
use warp::{Filter, http::Response};
use std::{convert::Infallible, net::SocketAddr};

#[derive(Deserialize, Serialize)]
struct SearchUrlQuery {
    q: String,
}

async fn local_search<const N: usize>((query, index): (SearchUrlQuery, DocumentIndex<N>)) -> Result<impl warp::Reply, Infallible> {
    let query = match Query::parse(&query.q) {
        Ok(query) => query,
        Err(e) => {
            error!("Error parsing query:");
            e.print(&query.q);
            return Ok(Response::builder().status(400).body("Error parsing query".to_string()).unwrap());
        },
    };
    let mut results = Vec::new();
    let mut stream = index.search(Arc::new(query)).await;
    while let Some(result) = stream.next().await {
        results.push(result);
    }
    Ok(Response::builder().header("Content-Type", "application/json").body(serde_json::to_string(&results).unwrap()).unwrap())
}


pub struct SearchPark {
    search_controllers: RwLock<HashMap<usize, Vec<(DocumentResult, PeerId)>>>,
}

impl SearchPark {
    pub fn new() -> SearchPark {
        SearchPark {
            search_controllers: RwLock::new(HashMap::new()),
        }
    }

    pub async fn insert(self: Arc<Self>, controller: SearchController) -> usize {
        let id = rand::random();
        self.search_controllers.write().await.insert(id, Vec::new());
        tokio::spawn(async move {
            let mut controller = controller;
            while let Some((document, peer_id)) = controller.recv().await {
                self.search_controllers.write().await.entry(id).and_modify(|v| v.push((document, peer_id)));
            }
        });
        id
    }

    pub async fn get_results(self: Arc<Self>, id: usize) -> Vec<(DocumentResult, PeerId)> {
        std::mem::take(self.search_controllers.write().await.get_mut(&id).unwrap())
    }
}

async fn search((query, search_park, kamilata): (SearchUrlQuery, Arc<SearchPark>, NodeController)) -> Result<impl warp::Reply, Infallible> {
    let query = match Query::parse(&query.q) {
        Ok(query) => query,
        Err(e) => {
            error!("Error parsing query:");
            e.print(&query.q);
            return Ok(Response::builder().status(400).body("Error parsing query".to_string()).unwrap());
        },
    };
    let search_controler = kamilata.search(query).await;
    let id = search_park.insert(search_controler).await;

    Ok(Response::builder().header("Content-Type", "application/json").header("Access-Control-Allow-Origin", "*").body("{\"id\": ".to_string() + &id.to_string() + "}").unwrap())
}

#[derive(Deserialize, Serialize)]
struct FetchResultsQuery {
    id: usize,
}

async fn fetch_results((query, search_park): (FetchResultsQuery, Arc<SearchPark>)) -> Result<impl warp::Reply, Infallible> {
    let id = query.id;
    let search_results: Vec<_> = search_park.get_results(id).await.into_iter().map(|(d, p)| (d, p.to_string())).collect();
    Ok(Response::builder().header("Content-Type", "application/json").header("Access-Control-Allow-Origin", "*").body(serde_json::to_string(&search_results).unwrap()).unwrap())
}

pub async fn serve_api<const N: usize>(api_addr: &str, index: DocumentIndex<N>, search_park: Arc<SearchPark>, kamilata: NodeController) {
    let hello_world = warp::path::end().map(|| "Hello, World at root!");

    let local_search = warp::get()
        .and(warp::path("local-search"))
        .and(warp::query::<SearchUrlQuery>())
        .map(move |q: SearchUrlQuery| (q, index.clone()))
        .and_then(local_search);
    
    let search_park2 = Arc::clone(&search_park);
    let search = warp::get()
        .and(warp::path("search"))
        .and(warp::query::<SearchUrlQuery>())
        .map(move |q: SearchUrlQuery| (q, Arc::clone(&search_park2), kamilata.clone()))
        .and_then(search);

    let fetch_result = warp::get()
        .and(warp::path("fetch-results"))
        .and(warp::query::<FetchResultsQuery>())
        .map(move |id: FetchResultsQuery| (id, Arc::clone(&search_park)))
        .and_then(fetch_results);

    let routes = warp::get().and(
        hello_world
            .or(local_search)
            .or(search)
            .or(fetch_result)
    );

    warp::serve(routes).run(api_addr.parse::<SocketAddr>().unwrap()).await;
}
