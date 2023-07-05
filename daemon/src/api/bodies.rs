use super::*;

#[derive(Deserialize, Serialize)]
pub struct ApiSearchQuery {
    /// Raw search query
    pub q: String,
}

#[derive(Deserialize, Serialize)]
pub struct ApiSearchResponse {
    /// Unique search identifier to use in [ApiResultsQuery::id]
    pub id: u64,
    /// The query parsed from [ApiSearchQuery::q]
    pub query: Query,
}

#[derive(Deserialize, Serialize)]
pub struct ApiResultsQuery {
    /// Unique search identifier from [ApiSearchResponse::id]
    pub id: u64,
}
