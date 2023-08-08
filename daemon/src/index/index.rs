use super::*;

#[derive(Clone)]
pub struct DocumentIndex {
    config: Arc<Args>,
    inner: Arc<RwLock<DocumentIndexInner>>,
}

#[allow(dead_code)]
impl DocumentIndex {
    pub async fn new(config: Arc<Args>) -> DocumentIndex {
        DocumentIndex {
            inner: Arc::new(RwLock::new(DocumentIndexInner::new(Arc::clone(&config)).await)),
            config,
        }
    }

    pub async fn run(&self) {
        let f1 = self.refresh();
        let f2 = self.sweep();
        let f = futures::future::join(f1, f2);
        f.await;
    }

    pub async fn sweep(&self) {
        #[cfg(any(feature = "database-lmdb", feature = "database-mdbx"))]
        loop {
            sleep(Duration::from_secs(SWEEP_INTERVAL)).await;
            let mut inner = self.inner.write().await;
            inner.sweep().await; // TODO: optimize
        }
    }

    pub async fn refresh(&self) {
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
                    sleep(Duration::from_secs(REFRESH_INTERVAL)).await;
                    continue;
                }
            };
            last_printed_error = None;
            pinned.retain(|cid| already_explored.insert(cid.clone()));
            if pinned.is_empty() {
                sleep(Duration::from_secs(REFRESH_INTERVAL)).await;
                continue;
            }
            debug!("{} new pinned elements", pinned.len());
            let start = Instant::now();

            // Explore directories and fetch prioritized documents
            let mut listed_folders: HashSet<String> = self.folders().await.keys().cloned().collect();
            let mut fetched_documents: HashSet<String> = self.documents().await;
            let mut unprioritized_documents: HashSet<String> = HashSet::new();
            let mut prev_document_count = fetched_documents.len();
            while let Some(parent_cid) = pinned.pop() {
                // FIXME: top level files are ignored later
                if !listed_folders.insert(parent_cid.clone()) {
                    continue;
                }
                
                // Get content
                let new_links = match ls(ipfs_rpc, parent_cid.clone()).await {
                    Ok(new_links) => new_links,
                    Err(e) => {
                        warn!("Error listing potential directory: {e:?}");
                        continue;
                    },
                };

                // Handle content
                for (child_cid, child_name, child_is_folder) in new_links {
                    let Ok(child_cid) = Cid::try_from(child_cid.as_str()) else {continue};
                    let Ok(child_cid) = child_cid.into_v1() else {continue};
                    let child_cid = child_cid.to_string();

                    if child_is_folder {
                        if !listed_folders.contains(&child_cid) {
                            pinned.push(child_cid.clone());
                        }
                        self.add_ancestor(&child_cid, child_name, &parent_cid).await;
                    } else if !fetched_documents.contains(&child_cid) && child_name.ends_with(".html") {
                        let document = match fetch_document(ipfs_rpc, &child_cid).await {
                            Ok(document) => Some(document),
                            Err(e) => {
                                warn!("Error while fetching document: {e:?}");
                                None
                            },
                        };
                        fetched_documents.insert(child_cid.clone());
                        if fetched_documents.len() % 500 == 0 {
                            debug!("{} documents yet ({} fetched) ({:02}s)", fetched_documents.len(), self.document_count().await, start.elapsed().as_secs_f32());
                        }
                        if let Some(document) = document {
                            if let Some(inspected) = inspect_document(document) {
                                self.add_document(&child_cid, inspected).await;
                                self.add_ancestor(&child_cid, child_name, &parent_cid).await;
                            }
                        }
                    } else {
                        unprioritized_documents.insert(child_cid.clone());
                    }
                }
            }
            let mut document_count = self.document_count().await;
            if prev_document_count != document_count {
                debug!("{} documents (+{} in {:02}s)", document_count, document_count - prev_document_count, start.elapsed().as_secs_f32());
                prev_document_count = document_count;
            }

            // Fetch remaining documents (low priority)
            trace!("Fetching {} unprioritized documents", unprioritized_documents.len());
            /*for cid in unprioritized_documents {
                let document = match fetch_document(ipfs_rpc, &cid).await {
                    Ok(Some(document)) => document,
                    Ok(None) => continue,
                    Err(e) => {
                        warn!("Error while fetching document: {e:?}");
                        continue;
                    },
                };
                self.add_document(cid.clone(), document).await;
            }
            document_count = self.document_count().await;
            if prev_document_count != document_count {
                debug!("{} documents (+{} in {:02}s)", document_count, document_count - prev_document_count, start.elapsed().as_secs_f32());
            }*/
            
            self.update_filter().await;
            debug!("Filter filled at {:.04}% ({:02}s)", self.get_filter().await.load()*100.0, start.elapsed().as_secs_f32());

            sleep(Duration::from_secs(REFRESH_INTERVAL)).await;
        }
    }

    pub async fn folders(&self) -> HashMap<String, usize> {
        self.inner.read().await.folders()
    }

    pub async fn documents(&self) -> HashSet<String> {
        self.inner.read().await.documents()
    }

    pub async fn document_count(&self) -> usize {
        self.inner.read().await.document_count()
    }

    pub async fn add_document(&self, cid: &String, doc: DocumentInspectionReport) {
        self.inner.write().await.add_document(cid, doc).await;
    }

    pub async fn add_ancestor(&self, cid: &String, name: String, folder_cid: &String) {
        self.inner.write().await.add_ancestor(cid, name, folder_cid);
    }

    pub async fn add_ancestors(&self, ancestors: Vec<(&String, String, &String)>) {
        let mut inner = self.inner.write().await;
        for (cid, name, folder_cid) in ancestors {
            inner.add_ancestor(cid, name, folder_cid);
        }
    }

    pub async fn build_path(&self, cid: &String) -> Option<Vec<Vec<String>>> {
        self.inner.read().await.build_path(cid)
    }

    pub async fn update_filter(&self) {
        self.inner.write().await.update_filter().await;
    }
}


#[async_trait]
impl Store<FILTER_SIZE> for DocumentIndex {
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
        vec![result % (FILTER_SIZE * 8)]
    }

    async fn get_filter(&self) -> Filter<FILTER_SIZE> {
        self.inner.read().await.filter.clone()
    }

    fn search(&self, query: Arc<Query>) -> ResultStreamBuilderFut<DocumentResult> {
        let inner2 = Arc::clone(&self.inner);

        Box::pin(async move {
            #[cfg(any(feature = "database-lmdb", feature = "database-mdbx"))]
            let res = inner2.write().await.search(query).await;
    
            #[cfg(not(any(feature = "database-lmdb", feature = "database-mdbx")))]
            let res = inner2.read().await.search(query).await;

            res
        })
    }
}
