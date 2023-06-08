use crate::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentResult {
    cid: String,
    icon_cid: Option<String>,
    domain: Option<String>,
    title: String,
    description: String,
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
