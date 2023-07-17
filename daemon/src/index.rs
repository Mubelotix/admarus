use bimap::BiHashMap;
use std::hash::{Hash, Hasher};
use crate::prelude::*;

const REFRESH_PINNED_INTERVAL: u64 = 120;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct LocalCid(u32);
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

struct DocumentIndexInner<const N: usize> {
    config: Arc<Args>,
    filter: Filter<N>,
    filter_needs_update: bool,

    ancestors: HashMap<LocalCid, HashMap<LocalCid, String>>,
    folders: HashSet<LocalCid>,

    cid_counter: u32,
    cids: BiHashMap<LocalCid, String>,

    index: HashMap<String, HashMap<LocalCid, f32>>,
    filters: HashMap<(String, String), Vec<LocalCid>>,
}

impl<const N: usize> DocumentIndexInner<N> {
    pub fn new(config: Arc<Args>) -> DocumentIndexInner<N> {
        DocumentIndexInner {
            config,
            filter: Filter::new(),
            filter_needs_update: false,

            ancestors: HashMap::new(),
            folders: HashSet::new(),

            cids: BiHashMap::new(),
            cid_counter: 0,

            index: HashMap::new(),
            filters: HashMap::new(),
        }
    }

    pub fn folders(&self) -> HashMap<String, usize> {
        let mut lfolders = HashMap::new();
        for ancestors in self.ancestors.values() {
            for ancestor in ancestors.keys() {
                *lfolders.entry(ancestor.to_owned()).or_default() += 1;
            }
        }

        let mut folders = HashMap::new();
        for (lcid, count) in lfolders {
            if let Some(cid) = self.cids.get_by_left(&lcid) {
                folders.insert(cid.to_owned(), count);
            }
        }
        
        folders
    }

    pub fn documents(&self) -> HashSet<String> {
        self.cids
            .iter()
            .filter(|(lcid, _)| !self.folders.contains(lcid))
            .map(|(_, cid)| cid.to_owned())
            .collect()
    }

    pub fn document_count(&self) -> usize {
        self.cids.len() - self.folders.len()
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

    pub fn add_document(&mut self, cid: &String, document: Document) {
        if self.cids.contains_right(cid) {
            warn!("Tried to add already indexed document: {cid}");
            return;
        }

        // Store cid
        let lcid = LocalCid(self.cid_counter);
        self.cid_counter += 1;
        self.cids.insert(lcid, cid.to_owned());
        self.folders.remove(&lcid);

        // Index by words
        let (words, filters) = document.into_parts();
        let word_count = words.len() as f64;
        for word in words {
            let frequencies = self.index.entry(word.clone()).or_default();
            *frequencies.entry(lcid).or_insert(0.) += 1. / word_count as f32;
            self.filter.add_word::<DocumentIndex<N>>(&word);
        }
        
        // Index by filters
        for (key, value) in filters {
            self.filters.entry((key.to_string(), value.clone())).or_default().push(lcid);
            self.filter.add_word::<DocumentIndex<N>>(&format!("{key}={value}"));
        }
    }

    pub fn add_ancestor(&mut self, cid: &String, name: String, folder_cid: &String) {
        let lcid = match self.cids.get_by_right(cid) {
            Some(lcid) => lcid.to_owned(),
            None => {
                let lcid = LocalCid(self.cid_counter);
                self.cid_counter += 1;
                self.cids.insert(lcid, cid.clone());
                self.folders.insert(lcid);
                lcid
            }
        };

        let ancestor_lcid = match self.cids.get_by_right(folder_cid) {
            Some(lcid) => lcid.to_owned(),
            None => {
                let lcid = LocalCid(self.cid_counter);
                self.cid_counter += 1;
                self.cids.insert(lcid, folder_cid.clone());
                lcid
            }
        };
        self.folders.insert(ancestor_lcid);

        self.ancestors.entry(lcid).or_default().insert(ancestor_lcid, name);
    }

    pub fn build_path(&self, cid: &String) -> Option<Vec<Vec<String>>> {
        let lcid = match self.cids.get_by_right(cid) {
            Some(lcid) => lcid.to_owned(),
            None => {
                warn!("Tried to build path for unknown cid: {cid}");
                return None;
            },
        };

        // List initial paths that will be explored
        let mut current_paths: Vec<(LocalCid, Vec<String>)> = Vec::new();
        for (ancestor, name) in self.ancestors.get(&lcid)? {
            current_paths.push((ancestor.to_owned(), vec![name.to_owned()]));
        }

        // Expand known paths and keep track of them all
        let mut paths: Vec<(LocalCid, Vec<String>)> = Vec::new();
        while let Some(current_path) = current_paths.pop() {
            if let Some(ancestors) = self.ancestors.get(&current_path.0) {
                for (ancestor, name) in ancestors {
                    if name.is_empty() {
                        continue;
                    }
                    let mut new_path = current_path.clone();
                    new_path.0 = ancestor.to_owned();
                    new_path.1.insert(0, name.to_owned());
                    current_paths.push(new_path);
                }
            }
            paths.push(current_path);
        }

        // Resolve the root cid to build final paths
        let mut final_paths = Vec::new();
        for (root, mut path) in paths {
            if let Some(first) = path.first() {
                if first.starts_with("dns-pin-") {
                    let dns_pin_with_suffix = first.split_at(8).1;
                    if let Some(i) = dns_pin_with_suffix.bytes().rposition(|c| c == b'-') {
                        let dns_pin = dns_pin_with_suffix.split_at(i).0;
                        let (domain, path_start) = dns_pin.split_once('/').unwrap_or((dns_pin, "/"));
                        let (domain, path_start) = (domain.to_owned(), path_start.to_owned());
                        path[0] = domain;
                        for path_part in path_start.split('/').rev() {
                            if !path_part.is_empty() {
                                path.insert(1, path_part.to_owned());
                            }
                        }
                        final_paths.push(path);
                        continue;
                    }
                }
            }
            let root_cid = match self.cids.get_by_left(&root) {
                Some(root_cid) => root_cid.to_owned(),
                None => match self.cids.get_by_left(&root) {
                    Some(root_cid) => root_cid.to_owned(),
                    None => continue,
                },
            };
            path.insert(0, root_cid);
            final_paths.push(path);
        }

        Some(final_paths)
    }

    // TODO: switching self to static may improve performance by a lot
    pub async fn search(&self, query: Arc<Query>) -> ResultStream<DocumentResult> {
        let matching_docs = match query.match_score(&self.filter) > 0 {
            true => query.matching_docs(&self.index, &self.filters),
            false => Vec::new(),
        };

        let futures = matching_docs
            .into_iter()
            .filter_map(|lcid| self.cids.get_by_left(&lcid))
            .map(|cid| (cid, self.build_path(cid).unwrap_or_default()))
            .map(|(cid, paths)| cid_to_result_wrapper(Arc::clone(&query), cid.to_owned(), paths, Arc::clone(&self.config)))
            .collect();

        Box::pin(DocumentResultStream { futures })
    }
}

#[derive(Clone)]
pub struct DocumentIndex<const N: usize> {
    config: Arc<Args>,
    inner: Arc<RwLock<DocumentIndexInner<N>>>,
}

#[allow(dead_code)]
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
            let mut listed_folders: HashSet<String> = self.documents().await;
            let mut fetched_documents: HashSet<String> = listed_folders.clone();
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
                            Ok(document) => document,
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
                            self.add_document(&child_cid, document).await;
                            self.add_ancestor(&child_cid, child_name, &parent_cid).await;
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

            sleep(Duration::from_secs(REFRESH_PINNED_INTERVAL)).await;
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

    pub async fn add_document(&self, cid: &String, document: Document) {
        self.inner.write().await.add_document(cid, document);
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

async fn cid_to_result(query: Arc<Query>, cid: String, paths: Vec<Vec<String>>, config: Arc<Args>) -> Option<DocumentResult> {
    let Ok(Some(document)) = fetch_document(&config.ipfs_rpc, &cid).await else {return None};
    let Some(result) = document.into_result(paths, &query) else {return None};
    Some(result)
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
