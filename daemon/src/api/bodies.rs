use super::*;

#[derive(Deserialize, Serialize)]
pub(super) struct SearchUrlQuery {
    pub q: String,
}


#[derive(Deserialize, Serialize)]
pub(super) struct FetchResultsQuery {
    pub id: usize,
}
