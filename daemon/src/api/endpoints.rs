use super::*;

pub(super) async fn local_search<const N: usize>((query, index): (ApiSearchQuery, DocumentIndex<N>)) -> Result<impl warp::Reply, Infallible> {
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

pub(super) async fn search((query, search_park, kamilata): (ApiSearchQuery, Arc<SearchPark>, NodeController)) -> Result<impl warp::Reply, Infallible> {
    let query = match Query::parse(&query.q) {
        Ok(query) => query,
        Err(e) => {
            error!("Error parsing query:");
            e.print(&query.q);
            return Ok(Response::builder().status(400).header("Access-Control-Allow-Origin", "*").body("Error parsing query".to_string()).unwrap());
        },
    };
    info!("Searching for {:?}", query);
    let search_controler = kamilata.search(query.clone()).await;
    let id = search_park.insert(search_controler).await;

    let resp = Response::builder()
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(serde_json::to_string(&ApiSearchResponse {
            id: id as u64,
            query,
        }).unwrap())
        .unwrap();
    Ok(resp)
}

pub(super) async fn fetch_results((query, search_park): (ApiResultsQuery, Arc<SearchPark>)) -> Result<impl warp::Reply, Infallible> {
    let id = query.id as usize;
    let search_results = match search_park.fetch_results(id).await {
        Some(search_results) => search_results,
        None => return Ok(Response::builder().status(400).header("Access-Control-Allow-Origin", "*").body("Search not found".to_string()).unwrap()),
    };
    let search_results = search_results.into_iter().map(|(d, p)| (d, p.to_string())).collect::<Vec<_>>();
    Ok(Response::builder().header("Content-Type", "application/json").header("Access-Control-Allow-Origin", "*").body(serde_json::to_string(&search_results).unwrap()).unwrap())
}
