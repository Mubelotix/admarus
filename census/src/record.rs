use crate::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, Hashable)]
pub struct Record {
    pub peer_id: String,
    pub addrs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbRecord {
    pub r: Record,
    pub ts: u64,
}

impl AsRef<Record> for DbRecord {
    fn as_ref(&self) -> &Record { &self.r }
}
