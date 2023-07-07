use crate::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, Hashable)]
pub struct Record {
    pub peer_id: String,
    pub addrs: Vec<String>,
    /// Aims to allow census to count documents.
    /// Since we can't handle the list of files (think about how wikipedia alone has 6M+ pages), this contains folders.
    /// Each folder has a CID, and the count of files it indexes in that folder (non-recursive).
    pub folders: Vec<(String, u64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbRecord {
    pub r: Record,
    pub ts: u64,
}

impl AsRef<Record> for DbRecord {
    fn as_ref(&self) -> &Record { &self.r }
}
