use super::*;

pub(super) async fn version() -> Result<impl warp::Reply, Infallible> {
    Ok(Response::builder()
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&ApiVersionResponse { version: 0 }).unwrap())
        .unwrap())
}
