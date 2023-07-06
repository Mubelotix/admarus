use crate::prelude::*;

const REFRESH_PINNED_INTERVAL: u64 = 120;

struct DocumentIndexInner<const N: usize> {
    config: Arc<Args>,
    filter: Filter<N>,
    filter_needs_update: bool,

    /// This reduces RAM usage as we can now store u32 instead of Strings in the index.
    /// Saves 2.5kB per document in average.
    ids: HashMap<u32, String>,
    id_counter: u32,

    metadata: HashMap<String, Metadata>,

    /// word -> [cid -> frequency]
    index: HashMap<String, HashMap<u32, f64>>,
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

    pub fn documents(&self) -> HashSet<String> {
        self.metadata.keys().cloned().collect()
    }

    pub fn document_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn metadata(&self) -> HashMap<String, Metadata> {
        self.metadata.clone()
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

        let futures = matching_docs
            .into_iter()
            .filter_map(|cid|
                self.metadata.get(&cid).map(|metadata|
                    (cid, metadata.to_owned())
                )
            )
            .map(|(cid, metadata)| cid_to_result_wrapper(Arc::clone(&query), cid, metadata, Arc::clone(&self.config)))
            .collect();

        Box::pin(DocumentResultStream { futures })
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
        let ipfs_rpc = &self.config.ipfs_rpc;
        loop {
            // List pinned elements
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

            // Explore directories and fetch prioritized documents
            let mut metadatas: HashMap<String, Metadata> = self.metadata().await;
            let mut fetched_documents: HashSet<String> = self.documents().await;
            let mut prev_document_count = fetched_documents.len();
            while let Some(cid) = pinned.pop() {
                let metadata = metadatas.get(&cid);
                // FIXME: top level files are ignored later

                match ls(ipfs_rpc, cid, metadata).await {
                    Ok(new_links) => {
                        for (cid, metadata) in new_links {
                            if !metadata.is_file && !metadatas.contains_key(&cid) {
                                pinned.push(cid.clone());
                            }
                            if metadata.is_file && !fetched_documents.contains(&cid) && metadata.paths.iter().any(|p| p.last().map(|p| p.ends_with(".html")).unwrap_or(false)) {
                                let document = match fetch_document(ipfs_rpc, &cid).await {
                                    Ok(document) => document,
                                    Err(e) => {
                                        warn!("Error while fetching document: {e:?}");
                                        None
                                    },
                                };
                                fetched_documents.insert(cid.clone());
                                if let Some(document) = document {
                                    self.add_document(cid.clone(), document, metadata.clone()).await;
                                }
                            }
                            // FIXME: when already scanned, we miss paths for children because we don't rescan
                            metadatas.entry(cid).or_default().merge(metadata);
                        }
                    }
                    Err(e) => warn!("Error listing potential directory: {e:?}"),
                }
            }
            let mut document_count = self.document_count().await;
            if prev_document_count != document_count {
                debug!("{} documents (+{} in {:02}s)", document_count, document_count - prev_document_count, start.elapsed().as_secs_f32());
                prev_document_count = document_count;
            }

            // Fetch remaining documents (low priority)
            for (cid, metadata) in metadatas {
                if !fetched_documents.contains(&cid) {
                    let document = match fetch_document(ipfs_rpc, &cid).await {
                        Ok(Some(document)) => document,
                        Ok(None) => continue,
                        Err(e) => {
                            warn!("Error while fetching document: {e:?}");
                            continue;
                        },
                    };
                    self.add_document(cid.clone(), document, metadata).await;
                }
            }
            document_count = self.document_count().await;
            if prev_document_count != document_count {
                debug!("{} documents (+{} in {:02}s)", document_count, document_count - prev_document_count, start.elapsed().as_secs_f32());
            }
            
            self.update_filter().await;
            debug!("Filter filled at {:.04}% ({:02}s)", self.get_filter().await.load()*100.0, start.elapsed().as_secs_f32());

            sleep(Duration::from_secs(REFRESH_PINNED_INTERVAL)).await;
        }
    }

    pub async fn documents(&self) -> HashSet<String> {
        self.inner.read().await.documents()
    }

    pub async fn document_count(&self) -> usize {
        self.inner.read().await.document_count()
    }

    pub async fn metadata(&self) -> HashMap<String, Metadata> {
        self.inner.read().await.metadata()
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

async fn cid_to_result(query: Arc<Query>, cid: String, metadata: Metadata, config: Arc<Args>) -> Option<DocumentResult> {
    let Ok(Some(document)) = fetch_document(&config.ipfs_rpc, &cid).await else {return None};
    let Some(result) = document.into_result(metadata.to_owned(), &query) else {return None};
    Some(result)
}

fn cid_to_result_wrapper(query: Arc<Query>, cid: String, metadata: Metadata, config: Arc<Args>) -> Pin<Box<dyn Future<Output = Option<DocumentResult>> + Send>> {
    Box::pin(cid_to_result(query, cid, metadata, config))
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
