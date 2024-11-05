

use std::hash::{Hash, Hasher};
use crate::prelude::*;

const REFRESH_INTERVAL: u64 = 100;
const SWEEP_INTERVAL: u64 = 30;

mod index;
mod status;
pub use index::*;
pub use status::*;

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

fn cid_to_result_wrapper(query: Arc<Query>, cid: String, paths: Vec<Vec<String>>, config: Arc<Args>) -> Pin<Box<dyn Future<Output = Option<DocumentResult>> + Send>> {
    Box::pin(cid_to_result(query, cid, paths, config))
}

struct DocumentResultStream {
    futures: Vec<Pin<Box<dyn Future<Output = Option<DocumentResult>> + Send>>>,
}

impl Stream for DocumentResultStream {
    type Item = DocumentResult;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        match self.futures.last_mut() {
            Some(fut) => {
                match fut.as_mut().poll(cx) {
                    std::task::Poll::Ready(Some(r)) => {
                        self.futures.pop();
                        std::task::Poll::Ready(Some(r))
                    },
                    std::task::Poll::Ready(None) => {
                        self.futures.pop();
                        self.poll_next(cx)
                    },
                    std::task::Poll::Pending => std::task::Poll::Pending,
                }
            },
            None => std::task::Poll::Ready(None),
        }
    }
}
