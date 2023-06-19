use crate::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, Hashable)]
pub struct Record {
    pub peer_id: String,
    pub addrs: Vec<String>,
}
