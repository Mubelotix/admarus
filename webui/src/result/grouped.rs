use crate::prelude::*;

impl DocumentResult {
    pub fn root_id(&self) -> &str {
        self.paths.first().and_then(|p| p.first()).map(|r| r.as_str()).unwrap_or_default()
    }
}

// TODO: switch to references
pub struct GroupedResults {
    pub first: (String, Scores),
    pub children: Vec<(String, Scores)>, // TODO remove pub
}

impl GroupedResults {
    pub fn new(first: (String, Scores)) -> Self {
        Self {
            first,
            children: Vec::new(),
        }
    }

    pub fn insert(&mut self, cid: String, scores: Scores) {
        let i = self.children.binary_search_by_key(&&scores, |(_,s)| s).unwrap_or_else(|i| i);
        self.children.insert(i, (cid, scores));
    }

    // Returns the best [Scores] it contains
    pub fn scores(&self) -> Scores {
        std::cmp::min(self.first.1.clone(), self.children.first().map(|(_,s)| s.to_owned()).unwrap_or(Scores::zero()))
    }

    pub fn to_docs(&self, results: &HashMap<String, DocumentResult>) -> Option<Vec<(DocumentResult, Scores)>> {
        let results = std::iter::once(&self.first).chain(self.children.iter()).filter_map(|(cid, scores)| {
            results.get(cid).map(|r| (r.to_owned(), scores.to_owned()))
        }).collect::<Vec<_>>();
        match results.is_empty() {
            true => None,
            false => Some(results)
        }
    }
}
