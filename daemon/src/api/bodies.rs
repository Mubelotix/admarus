use super::*;

#[derive(Deserialize, Serialize)]
pub(super) struct ApiSearchQuery {
    /// Raw search query
    pub q: String,
}

#[derive(Deserialize, Serialize)]
pub(super) struct ApiSearchResponse {
    /// Unique search identifier to use in [ApiResultsQuery::id]
    pub id: usize,
    /// The query parsed from [ApiSearchQuery::q]
    pub query: Query,
}

#[derive(Deserialize, Serialize)]
pub(super) struct ApiResultsQuery {
    /// Unique search identifier from [ApiSearchResponse::id]
    pub id: usize,
}
