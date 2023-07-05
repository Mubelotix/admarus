use crate::prelude::*;

impl SearchResult for DocumentResult {
    type Cid = String;
    type ParsingError = serde_json::Error;

    fn cid(&self) -> Self::Cid {
        self.cid.clone()
    }

    fn into_bytes(self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap_or_default()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::ParsingError> {
        serde_json::from_slice(bytes)
    }
}
