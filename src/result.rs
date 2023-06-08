use crate::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentResult {
    pub cid: String,
    pub icon_cid: Option<String>,
    pub domain: Option<String>,
    pub title: String,
    pub description: String,
}

impl SearchResult for DocumentResult {
    type Cid = String;

    fn cid(&self) -> Self::Cid {
        self.cid.clone()
    }

    fn into_bytes(self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap()
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        serde_json::from_slice(bytes).unwrap()
    }
}
