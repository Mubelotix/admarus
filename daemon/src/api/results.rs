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
