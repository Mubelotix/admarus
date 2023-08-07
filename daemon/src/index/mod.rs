

pub(self) use bimap::BiHashMap;
pub(self) use std::hash::{Hash, Hasher};
pub(self) use crate::prelude::*;
pub(self) const REFRESH_PINNED_INTERVAL: u64 = 120;

mod index;
pub use index::*;

#[cfg(any(feature = "database-lmdb", feature = "database-mdbx"))]
mod inner_db;
#[cfg(any(feature = "database-lmdb", feature = "database-mdbx"))]
pub(self) use inner_db::*;

#[cfg(not(any(feature = "database-lmdb", feature = "database-mdbx")))]
mod inner_im;
#[cfg(not(any(feature = "database-lmdb", feature = "database-mdbx")))]
pub(self) use inner_im::*;


#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct LocalCid(pub u32);
impl Hash for LocalCid {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.0, state)
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct LocalDid(u32);
impl Hash for LocalDid {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.0, state)
    }
}

pub async fn cid_to_result(query: Arc<Query>, cid: String, paths: Vec<Vec<String>>, config: Arc<Args>) -> Option<DocumentResult> {
    let Ok(raw) = fetch_document(&config.ipfs_rpc, &cid).await else {return None};
    generate_result(raw, cid, &query, paths)
}
