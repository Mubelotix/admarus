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
