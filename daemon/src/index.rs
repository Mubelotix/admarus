use std::{sync::Arc, collections::HashSet};
use crate::prelude::*;

const REFRESH_PINNED_INTERVAL: u64 = 120;

struct DocumentIndexInner<const N: usize> {
    config: Arc<Args>,
    pub filter: Filter<N>,
    filter_needs_update: bool,

    /// This reduces RAM usage as we can now store u32 instead of Strings in the index.
    /// Saves 2.5kB per document in average.
    ids: HashMap<u32, String>,
    id_counter: u32,

    metadata: HashMap<String, Metadata>,

    /// word -> [cid -> frequency]
    pub index: HashMap<String, HashMap<u32, f64>>, // FIXME: no field should be public
    filters: HashMap<(String, String), Vec<u32>>,
}

impl<const N: usize> DocumentIndexInner<N> {
    pub fn new(config: Arc<Args>) -> DocumentIndexInner<N> {
        DocumentIndexInner {
            config,
            filter: Filter::new(),
            filter_needs_update: false,
            ids: HashMap::new(),
            id_counter: 0,
            metadata: HashMap::new(),
            index: HashMap::new(),
            filters: HashMap::new(),
        }
    }

    pub fn update_filter(&mut self) {
        if !self.filter_needs_update {
            return;
        }
        self.filter = Filter::new();
        for word in self.index.keys() {
            self.filter.add_word::<DocumentIndex<N>>(word);
        }
        self.filter_needs_update = false;
    }

    pub fn remove_document(&mut self, cid: &str) {
        let id = self.ids.iter().find(|(_, c)| *c == cid).map(|(id, _)| *id).unwrap();
        self.ids.remove(&id);
        self.metadata.remove(cid);

        // Remove from index
        for frequencies in self.index.values_mut() {
            frequencies.remove(&id);
        }
        let prev_index_len = self.index.len();
        self.index.retain(|_, frequencies| !frequencies.is_empty());

        // Remove from filters
        for ids in self.filters.values_mut() {
            ids.retain(|i| *i != id);
        }
        let prev_filters_len = self.filters.len();
        self.filters.retain(|_, ids| !ids.is_empty());

        // Update filter if necessary
        if prev_index_len != self.index.len() || prev_filters_len != self.filters.len() {
            self.filter_needs_update = true;
        }
    }

    pub fn add_document(&mut self, cid: String, document: Document, metadata: Metadata) {
        if self.metadata.contains_key(&cid) {
            return;
        }
        self.metadata.insert(cid.clone(), metadata);

        let (words, filters) = document.into_parts();
        let word_count = words.len() as f64;

        let id = self.id_counter;
        self.id_counter += 1;
        self.ids.insert(id, cid);

        for word in words {
            let frequencies = self.index.entry(word.clone()).or_default();
            *frequencies.entry(id).or_insert(0.) += 1. / word_count;
            self.filter.add_word::<DocumentIndex<N>>(&word);
        }
        
        for (key, value) in filters {
            self.filters.entry((key.to_string(), value.clone())).or_default().push(id);
            self.filter.add_word::<DocumentIndex<N>>(&format!("{key}={value}"));
        }
    }

    pub fn add_documents(&mut self, documents: Vec<(String, Document, Metadata)>) {
        for (cid, document, link) in documents {
            self.add_document(cid, document, link);
        }
    }

    // TODO: switching self to static may improve performance by a lot
    pub async fn search(&self, query: Arc<Query>) -> ResultStream<DocumentResult> {
        let matching_docs = match query.match_score(&self.filter) > 0 {
            true => query.matching_docs(&self.index, &self.filters).into_iter().map(|id| self.ids.get(&id).unwrap().to_owned()).collect::<Vec<_>>(),
            false => Vec::new(),
        };

        async fn cid_to_result(query: Arc<Query>, cid: String, metadata: Metadata, config: Arc<Args>) -> Option<DocumentResult> {
            let Ok(Some(document)) = fetch_document(&config.ipfs_rpc, &cid).await else {return None};
            let Some(result) = document.into_result(metadata.to_owned(), &query) else {return None};
            Some(result)
        }

        let stream: FuturesUnordered<_> = matching_docs
            .into_iter()
            .filter_map(|cid|
                self.metadata.get(&cid).map(|metadata|
                    cid_to_result(Arc::clone(&query), cid, metadata.to_owned(), Arc::clone(&self.config))
                )
            ).collect();
        
        Box::pin(stream.filter_map(|r| async move {r}))
    }
}

#[derive(Clone)]
pub struct DocumentIndex<const N: usize> {
    config: Arc<Args>,
    inner: Arc<RwLock<DocumentIndexInner<N>>>,
}

impl <const N: usize> DocumentIndex<N> {
    pub fn new(config: Arc<Args>) -> DocumentIndex<N> {
        DocumentIndex {
            inner: Arc::new(RwLock::new(DocumentIndexInner::new(Arc::clone(&config)))),
            config,
        }
    }

    pub async fn run(&self) {
        let mut already_explored = HashSet::new();
        let mut last_printed_error = None;
        loop {
            let mut pinned = match list_pinned(&self.config.ipfs_rpc).await {
                Ok(pinned) => pinned,
                Err(e) => {
                    let e_string = e.to_string();
                    if !last_printed_error.map(|lpe| lpe==e_string).unwrap_or(false) {
                        error!("Error while listing pinned elements: {}", e_string);
                    }
                    last_printed_error = Some(e_string);
                    sleep(Duration::from_secs(REFRESH_PINNED_INTERVAL)).await;
                    continue;
                }
            };
            last_printed_error = None;
            pinned.retain(|cid| already_explored.insert(cid.clone()));
            if pinned.is_empty() {
                sleep(Duration::from_secs(REFRESH_PINNED_INTERVAL)).await;
                continue;
            }
            debug!("{} new pinned elements", pinned.len());
            let start = Instant::now();
            
            let pinned_files = explore_all(&self.config.ipfs_rpc, pinned).await;
            debug!("{} new files", pinned_files.iter().filter(|(_,m)| m.is_file).count());

            let documents = collect_documents(&self.config.ipfs_rpc, pinned_files).await;
            debug!("{} new documents ({:02}s)", documents.len(), start.elapsed().as_secs_f32());

            self.add_documents(documents).await;
            self.update_filter().await;
            debug!("Filter filled at {:.04}%", self.get_filter().await.load()*100.0);

            sleep(Duration::from_secs(REFRESH_PINNED_INTERVAL)).await;
        }
    }

    pub async fn add_document(&self, cid: String, document: Document, link: Metadata) {
        self.inner.write().await.add_document(cid, document, link);
    }

    pub async fn add_documents(&self, documents: Vec<(String, Document, Metadata)>) {
        self.inner.write().await.add_documents(documents);
    }

    pub async fn remove_document(&self, cid: &str) {
        self.inner.write().await.remove_document(cid);
    }

    pub async fn update_filter(&self) {
        self.inner.write().await.update_filter();
    }
}


#[async_trait]
impl <const N: usize> Store<N> for DocumentIndex<N> {
    type Result = DocumentResult;
    type Query = Query;

    fn hash_word(word: &str) -> Vec<usize>  {
        let mut result = 1usize;
        const RANDOM_SEED: [usize; 16] = [542587211452, 5242354514, 245421154, 4534542154, 542866467, 545245414, 7867569786914, 88797854597, 24542187316, 645785447, 434963879, 4234274, 55418648642, 69454242114688, 74539841, 454214578213];
        for c in word.bytes() {
            for i in 0..8 {
                result = result.overflowing_mul(c as usize + RANDOM_SEED[i*2]).0;
                result = result.overflowing_add(c as usize + RANDOM_SEED[i*2+1]).0;
            }
        }
        vec![result % (N * 8)]
    }

    async fn get_filter(&self) -> Filter<N> {
        self.inner.read().await.filter.clone()
    }

    fn search(&self, query: Arc<Query>) -> ResultStreamBuilderFut<DocumentResult> {
        let inner2 = Arc::clone(&self.inner);
        Box::pin(async move {
            inner2.read().await.search(query).await
        })
    }
}
