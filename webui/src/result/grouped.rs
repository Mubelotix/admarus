use crate::prelude::*;

impl DocumentResult {
    pub fn root_id(&self) -> &str {
        self.paths.first().and_then(|p| p.first()).map(|r| r.as_str()).unwrap_or_default()
    }
}

#[derive(Default)]
pub struct GroupedResults {
    pub results: Vec<(String, Scores)>, // TODO remove pub
}

impl GroupedResults {
    pub fn insert(&mut self, cid: String, scores: Scores) {
        let i = self.results.binary_search_by_key(&&scores, |(_,s)| s).unwrap_or_else(|i| i);
        self.results.insert(i, (cid, scores));
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    pub fn scores(&self) -> Scores {
        self.results.first().map(|(_,s)| s.to_owned()).unwrap_or(Scores::zero())
    }
}
