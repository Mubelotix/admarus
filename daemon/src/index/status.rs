use serde::{Serialize, Deserialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct IndexingStatus {
    pub listed: usize,
    pub to_list: usize,
    pub loaded: usize,
    pub to_load: usize,
    pub to_load_unprioritized: usize,
    pub updating_filter: bool,
}
