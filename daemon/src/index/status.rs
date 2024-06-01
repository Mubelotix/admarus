use serde::Serialize;

#[derive(Default, Debug, Clone, Serialize)]
pub struct IndexingStatus {
    pub listed: usize,
    pub to_list: usize,
    pub loaded: usize,
    pub to_load: usize,
    pub to_load_unprioritized: usize,
    pub updating_filter: bool,
}
