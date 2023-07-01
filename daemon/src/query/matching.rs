use crate::prelude::*;

impl<const N: usize> SearchQuery<N> for Query {
    type ParsingError = serde_json::Error;

    fn match_score(&self, filter: &Filter<N>) -> u32 {
        todo!()
    }

    fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::ParsingError> {
        serde_json::from_slice(bytes)
    }
}

impl Query {
    pub fn match_score_doc(&self, doc: &Document) -> u32 {
        todo!()
    }

    pub fn matching_docs(&self, index: &HashMap<String, HashMap<String, f64>>) -> Vec<String> {
        todo!()
    }
}
