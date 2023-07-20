use super::*;

pub(super) async fn fetch_results((query, search_park): (ApiResultsQuery, Arc<SearchPark>)) -> Result<impl warp::Reply, Infallible> {
    let id = query.id as usize;
    let search_results = match search_park.fetch_results(id).await {
        Some(search_results) => search_results,
        None => return Ok(Response::builder().status(400).body("Search not found".to_string()).unwrap()),
    };
    let search_results = search_results.into_iter().map(|(d, p)| (d, p.to_string())).collect::<Vec<_>>();
    Ok(Response::builder().header("Content-Type", "application/json").body(serde_json::to_string(&search_results).unwrap()).unwrap())
}

pub(super) async fn get_result((q, search_park, config): (ApiResultQuery, Arc<SearchPark>, Arc<Args>)) -> Result<impl warp::Reply, Infallible> {
    let id = q.id as usize;
    let cid = q.cid;
    let query = match search_park.get_query(id).await {
        Some(query) => query,
        None => return Ok(Response::builder().status(400).body("Search not found".to_string()).unwrap()),
    };
    let result = cid_to_result(query, cid, Vec::new(), config).await;
    Ok(Response::builder().header("Content-Type", "application/json").body(serde_json::to_string(&result).unwrap()).unwrap())
}
