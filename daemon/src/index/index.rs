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
        let mut listed = HashSet::new();
        let mut loaded = self.documents().await;

        fn normalize_cid(cid: impl AsRef<str>) -> Option<String> {
            let cid = Cid::try_from(cid.as_ref()).ok()?;
            let cid = cid.into_v1().ok()?;
            Some(cid.to_string())
        }

        let mut last_printed_error = None;
        let ipfs_rpc = &self.config.ipfs_rpc;
        let mut previous_load = -1.0;
        loop {
            let mut to_list = Vec::new();
            let mut to_load = HashMap::new();
            let mut to_load_unprioritized = HashSet::new();

            // List pinned elements
            let pinned = match list_pinned(&self.config.ipfs_rpc).await {
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
            to_list.extend(pinned.iter().filter_map(normalize_cid).filter(|cid| !listed.contains(cid)));

            // Explore directories
            let start = Instant::now();
            let mut i = 0;
            if !to_list.is_empty() {debug!("{} elements to list", to_list.len())}
            while let Some(cid) = to_list.pop() {
                if !listed.insert(cid.clone()) {continue}
                let new_links = match ls(ipfs_rpc, cid.clone()).await {
                    Ok(new_links) => new_links,
                    Err(e) => {
                        warn!("Error listing potential directory: {e:?}");
                        continue;
                    },
                };
                for (child_cid, child_name, child_is_folder) in new_links {
                    let child_cid = normalize_cid(child_cid).unwrap();
                    if child_is_folder {
                        self.add_ancestor(&child_cid, child_name, &cid).await;
                        if !listed.contains(&child_cid) {
                            to_list.push(child_cid);
                        }
                    } else if !loaded.contains(&child_cid) {
                        if child_name.ends_with(".html") {
                            to_load.insert(child_cid, (child_name, cid.clone()));
                        } else if self.config.crawl_unprioritized {
                            to_load_unprioritized.insert((child_cid, child_name, cid.clone()));
                        }
                    } else {
                        self.add_ancestor(&child_cid, child_name, &cid).await;
                    }
                }
                to_list.sort();
                to_list.dedup();
                i += 1;
                if i % 500 == 0 {
                    debug!("Still listing pinned files ({i} in {:.02})", start.elapsed().as_secs_f32());
                }
            }

            // Load documents
            i = 0;
            if !to_load.is_empty() {debug!("{} documents to load ({:.02?}s)", to_load.len(), start.elapsed().as_secs_f32())}
            for (cid, (name, parent_cid)) in to_load.drain() {
                if !loaded.insert(cid.clone()) {continue}
                let Ok(document) = fetch_document(ipfs_rpc, &cid).await else {continue};
                let Some(inspected) = inspect_document(document) else {continue};
                self.add_document(&cid, inspected).await;
                self.add_ancestor(&cid, name, &parent_cid).await;
                i += 1;
                if i % 500 == 0 {
                    debug!("Still loading files ({i} in {:.02})", start.elapsed().as_secs_f32());
                }
            }

            // Load unprioritized documents
            i = 0;
            if !to_load_unprioritized.is_empty() {debug!("{} unprioritized documents to load ({:.02?}s)", to_load_unprioritized.len(), start.elapsed().as_secs_f32())};
            for (cid, name, parent_cid) in to_load_unprioritized.drain() {
                if !loaded.insert(cid.clone()) {continue}
                let Ok(document) = fetch_document(ipfs_rpc, &cid).await else {continue};
                let Some(inspected) = inspect_document(document) else {continue};
                self.add_document(&cid, inspected).await;
                self.add_ancestor(&cid, name, &parent_cid).await;
                i += 1;
                if i % 500 == 0 {
                    debug!("Still loading files ({i} in {:.02})", start.elapsed().as_secs_f32());
                }
            }
            
            self.update_filter().await;
            let load = self.get_filter().await.load()*100.0;
            if load != previous_load {
                previous_load = load;
                debug!("Filter filled at {load:.04}% ({:02}s)", start.elapsed().as_secs_f32());
            }
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
        self.inner.write().await.add_document(cid, doc);
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
