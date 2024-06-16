use super::*;

pub(super) async fn indexing_status(index: DocumentIndex) -> Result<impl warp::Reply, Infallible> {
    let status = index.status().await;
    let status_json = match serde_json::to_string(&status) {
        Ok(json) => json,
        Err(e) => {
            error!("Failed to serialize indexing status: {}", e);
            return Ok(Response::builder().status(500).body("Failed to serialize indexing status".to_string()).unwrap());
        }
    };
    Ok(Response::builder().header("Content-Type", "application/json").body(status_json).unwrap())
}
