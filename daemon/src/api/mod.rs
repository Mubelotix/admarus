use crate::prelude::*;
use libp2p::PeerId;
use warp::{Filter, http::Response};
use std::{convert::Infallible, net::SocketAddr};

mod bodies;
mod local_search;
mod search;
mod results;
use {
    local_search::*,
    bodies::*,
    search::*,
    results::*,
};

struct OngoingSearch {
    query: Query,
    results: Vec<(DocumentResult, PeerId)>,
    providers: HashMap<String, Vec<(PeerId, Vec<Multiaddr>)>>,
    last_fetch: Instant,
}

pub struct SearchPark {
    searches: RwLock<HashMap<usize, OngoingSearch>>,
    node: NodeController,
}

impl SearchPark {
    pub fn new(node: NodeController) -> SearchPark {
        SearchPark {
            searches: RwLock::new(HashMap::new()),
            node,
        }
    }

    pub async fn insert(self: Arc<Self>, query: Query, controller: SearchController) -> usize {
        let id = rand::random();
        self.searches.write().await.insert(id, OngoingSearch {
            query,
            results: Vec::new(),
            providers: HashMap::new(),
            last_fetch: Instant::now()
        });
        tokio::spawn(async move {
            let mut controller = controller;
            while let Some((document, peer_id)) = controller.recv().await {
                let addresses = self.node.addresses_of(peer_id).await;
                let mut searches = self.searches.write().await;
                let Some(search) = searches.get_mut(&id) else {break};
                search.providers.entry(document.cid.clone()).or_insert_with(Vec::new).push((peer_id, addresses));
                search.results.push((document, peer_id));
                if search.last_fetch.elapsed() > Duration::from_secs(60) {
                    searches.remove(&id);
                    trace!("Search {id} expired");
                    break;
                }
            }
        });
        id
    }

    pub async fn fetch_results(self: Arc<Self>, id: usize) -> Option<Vec<(DocumentResult, PeerId)>> {
        let mut searches = self.searches.write().await;
        let OngoingSearch { results, last_fetch, .. } =  searches.get_mut(&id)?;
        *last_fetch = Instant::now();
        Some(std::mem::take(results))
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

    let cors = warp::cors()
        .allow_origin("https://admarus.net")
        .allow_origin("http://localhost:8083")
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "DELETE"]);

    let routes = warp::any().and(
        hello_world
            .or(local_search)
            .or(search)
            .or(fetch_result)
    ).with(cors);

    warp::serve(routes).run(api_addr.parse::<SocketAddr>().unwrap()).await;
}
