use crate::prelude::*;
use libp2p::PeerId;
use warp::{Filter, http::Response};
use std::{convert::Infallible, net::SocketAddr};

mod bodies;
use bodies::*;

mod endpoints;
use endpoints::*;

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

pub async fn serve_api<const N: usize>(api_addr: &str, index: DocumentIndex<N>, search_park: Arc<SearchPark>, kamilata: NodeController) {
    let hello_world = warp::path::end().map(|| "Hello, World at root!");

    let local_search = warp::get()
        .and(warp::path("local-search"))
        .and(warp::query::<ApiSearchQuery>())
        .map(move |q: ApiSearchQuery| (q, index.clone()))
        .and_then(local_search);
    
    let search_park2 = Arc::clone(&search_park);
    let search = warp::get()
        .and(warp::path("search"))
        .and(warp::query::<ApiSearchQuery>())
        .map(move |q: ApiSearchQuery| (q, Arc::clone(&search_park2), kamilata.clone()))
        .and_then(search);

    let fetch_result = warp::get()
        .and(warp::path("fetch-results"))
        .and(warp::query::<ApiResultsQuery>())
        .map(move |id: ApiResultsQuery| (id, Arc::clone(&search_park)))
        .and_then(fetch_results);

    let routes = warp::get().and(
        hello_world
            .or(local_search)
            .or(search)
            .or(fetch_result)
    );

    warp::serve(routes).run(api_addr.parse::<SocketAddr>().unwrap()).await;
}
