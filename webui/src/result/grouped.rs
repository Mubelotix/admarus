use crate::prelude::*;

impl DocumentResult {
    pub fn root_id(&self) -> &str {
        self.paths.first().and_then(|p| p.first()).map(|r| r.as_str()).unwrap_or_default()
    }
}

// TODO: switch to references
#[derive(Default)]
pub struct GroupedResultRefs {
    pub results: Vec<(String, Scores)>, // TODO remove pub
}

impl GroupedResultRefs {
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

    pub fn to_docs(&self, results: &HashMap<String, DocumentResult>) -> Option<Vec<(DocumentResult, Scores)>> {
        let results = self.results.iter().filter_map(|(cid, scores)| {
            results.get(cid).map(|r| (r.to_owned(), scores.to_owned()))
        }).collect::<Vec<_>>();
        match results.is_empty() {
            true => None,
            false => Some(results)
        }
    }
}
