use crate::prelude::*;

pub async fn get_indexing_status(rpc_addr: &str) -> Result<IndexingStatus, ApiError> {
    get(format!("{rpc_addr}/indexing-status")).await
}
