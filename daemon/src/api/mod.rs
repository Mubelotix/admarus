use crate::prelude::*;
use libp2p::PeerId;
use warp::{Filter, http::Response};
use std::{convert::Infallible, net::SocketAddr};

mod bodies;
mod indexing_status;
mod local_search;
mod search;
mod results;
mod version;
use {
    bodies::*,
    indexing_status::*,
    local_search::*,
    search::*,
    results::*,
    version::*,
};

struct OngoingSearch {
    query: Arc<Query>,
    results: Vec<(DocumentResult, PeerId)>,
    providers: HashMap<String, HashSet<PeerId>>,
    last_fetch: Instant,
}

pub struct SearchPark {
    searches: RwLock<HashMap<usize, OngoingSearch>>,
}

impl SearchPark {
    pub fn new() -> SearchPark {
        SearchPark {
            searches: RwLock::new(HashMap::new()),
        }
    }

    pub async fn insert(self: Arc<Self>, query: Query, controller: SearchController) -> usize {
        let id = rand::random();
        self.searches.write().await.insert(id, OngoingSearch {
            query: Arc::new(query.clone()),
            results: Vec::new(),
            providers: HashMap::new(),
            last_fetch: Instant::now()
        });
        tokio::spawn(async move {
            let mut controller = controller;
            while let Some((result, peer_id)) = controller.recv().await {
                let Ok(result) = result.validate(&query) else {continue};
                let mut searches = self.searches.write().await;
                let Some(search) = searches.get_mut(&id) else {break};
                search.providers.entry(result.cid.clone()).or_insert_with(HashSet::new).insert(peer_id);
                search.results.push((result, peer_id));
                if search.last_fetch.elapsed() > Duration::from_secs(7) {
                    searches.remove(&id);
                    trace!("Search {id} expired");
                    break;
                }
            }
        });
        id
    }

    #[allow(clippy::map_clone)]
    pub async fn get_query(self: Arc<Self>, id: usize) -> Option<Arc<Query>> {
        let searches = self.searches.read().await;
        searches.get(&id).map(|s| Arc::clone(&s.query))
    }

    pub async fn fetch_results(self: Arc<Self>, id: usize) -> Option<Vec<(DocumentResult, PeerId)>> {
        let mut searches = self.searches.write().await;
        let OngoingSearch { results, last_fetch, .. } =  searches.get_mut(&id)?;
        *last_fetch = Instant::now();
        Some(std::mem::take(results))
    }
}

pub async fn serve_api(config: Arc<Args>, index: DocumentIndex, search_park: Arc<SearchPark>, kamilata: NodeController) {
    let hello_world = warp::path::end().map(|| "Hello, World at root!");

    let index2 = index.clone();
    let indexing_status = warp::get()
        .and(warp::path("indexing-status"))
        .map(move || index2.clone())
        .and_then(indexing_status);

    let local_search = warp::get()
        .and(warp::path("local-search"))
        .and(warp::query::<ApiSearchQuery>())
        .map(move |q: ApiSearchQuery| (q, index.clone()))
        .and_then(local_search);
    
    let search_park2 = Arc::clone(&search_park);
    let kamilata2 = kamilata.clone();
    let search = warp::get()
        .and(warp::path("search"))
        .and(warp::query::<ApiSearchQuery>())
        .map(move |q: ApiSearchQuery| (q, Arc::clone(&search_park2), kamilata2.clone()))
        .and_then(search);

    let search_park2 = Arc::clone(&search_park);
    let results = warp::get()
        .and(warp::path("results"))
        .and(warp::query::<ApiResultsQuery>())
        .map(move |id: ApiResultsQuery| (id, Arc::clone(&search_park2)))
        .and_then(fetch_results);

    // Backwards compatibility, this is the old endpoint
    let search_park2 = Arc::clone(&search_park);
    let fetch_results = warp::get()
        .and(warp::path("fetch-results"))
        .and(warp::query::<ApiResultsQuery>())
        .map(move |id: ApiResultsQuery| (id, Arc::clone(&search_park2)))
        .and_then(fetch_results);

    let config2 = Arc::clone(&config);
    let result = warp::get()
        .and(warp::path("result"))
        .and(warp::query::<ApiResultQuery>())
        .map(move |q: ApiResultQuery| (q, Arc::clone(&search_park), Arc::clone(&config2)))
        .and_then(get_result);

    let version = warp::get()
        .and(warp::path("version"))
        .and_then(version);

    let mut cors = warp::cors()
        .allow_headers(vec!["content-type"])
        .allow_methods(vec!["GET", "POST", "DELETE"]);
    for origin in &config.api_cors {
        cors = cors.allow_origin(origin.as_str());
    }

    let routes = warp::any().and(
        hello_world
            .or(indexing_status)
            .or(local_search)
            .or(search)
            .or(results)
            .or(fetch_results)
            .or(version)
            .or(result)
    ).with(cors);

    warp::serve(routes).run(config.api_addr.parse::<SocketAddr>().expect("Invalid api_addr")).await;
}
