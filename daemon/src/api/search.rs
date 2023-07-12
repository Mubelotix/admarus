use super::*;

pub(super) async fn search((query, search_park, kamilata): (ApiSearchQuery, Arc<SearchPark>, NodeController)) -> Result<impl warp::Reply, Infallible> {
    let query = match Query::parse(&query.q) {
        Ok(query) => query,
        Err(e) => {
            error!("Error parsing query:");
            e.print(&query.q);
            return Ok(Response::builder().status(400).body("Error parsing query".to_string()).unwrap());
        },
    };
    info!("Searching for {:?}", query);
    let search_controler = kamilata.search(query.clone()).await;
    let id = search_park.insert(query.clone(), search_controler).await;

    let resp = Response::builder()
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&ApiSearchResponse {
            id: id as u64,
            query,
        }).unwrap())
        .unwrap();
    Ok(resp)
}
