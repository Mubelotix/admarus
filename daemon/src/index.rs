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

    did_counter: u32,
    directories: BiHashMap<LocalDid, Vec<String>>, // TODO: allow making this an hashmap

    cid_counter: u32,
    cids: BiHashMap<LocalCid, String>,
    filenames: HashMap<LocalCid, Vec<(LocalDid, String)>>,

    index: HashMap<String, HashMap<LocalCid, f64>>,
    filters: HashMap<(String, String), Vec<LocalCid>>,
}

impl<const N: usize> DocumentIndexInner<N> {
    pub fn new(config: Arc<Args>) -> DocumentIndexInner<N> {
        DocumentIndexInner {
            config,
            filter: Filter::new(),
            filter_needs_update: false,

            directories: BiHashMap::new(),
            did_counter: 0,
            filenames: HashMap::new(),

            cids: BiHashMap::new(),
            cid_counter: 0,

            index: HashMap::new(),
            filters: HashMap::new(),
        }
    }

    pub fn documents(&self) -> HashSet<String> {
        self.cids.right_values().cloned().collect()
    }

    pub fn document_count(&self) -> usize {
        self.cids.len()
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

    pub fn add_document(&mut self, cid: String, document: Document, paths: Vec<Vec<String>>) {
        if self.cids.contains_right(&cid) {
            warn!("Tried to add already indexed document: {cid}");
            return;
        }

        // Store cid
        let lcid = LocalCid(self.cid_counter);
        self.cid_counter += 1;
        self.cids.insert(lcid, cid.clone());

        // Store paths
        self.add_paths(&cid, paths);

        // Index by words
        let (words, filters) = document.into_parts();
        let word_count = words.len() as f64;
        for word in words {
            let frequencies = self.index.entry(word.clone()).or_default();
            *frequencies.entry(lcid).or_insert(0.) += 1. / word_count;
            self.filter.add_word::<DocumentIndex<N>>(&word);
        }
        
        // Index by filters
        for (key, value) in filters {
            self.filters.entry((key.to_string(), value.clone())).or_default().push(lcid);
            self.filter.add_word::<DocumentIndex<N>>(&format!("{key}={value}"));
        }
    }

    pub fn add_paths(&mut self, cid: &String, paths: Vec<Vec<String>>) {
        let lcid = match self.cids.get_by_right(cid) {
            Some(lcid) => *lcid,
            None => {
                warn!("Tried to add paths for unknown document: {cid}");
                return;
            },
        };

        for mut path in paths.into_iter().filter(|p| !p.is_empty()) {
            let filename = path.remove(path.len()-1);
            let ldid = match self.directories.get_by_right(&path) {
                Some(dir_id) => *dir_id,
                None => {
                    let ldid = LocalDid(self.did_counter);
                    self.did_counter += 1;
                    self.directories.insert(ldid, path);
                    ldid
                },
            };
            self.filenames.entry(lcid).or_default().push((ldid, filename));
        }
    }

    // TODO: switching self to static may improve performance by a lot
    pub async fn search(&self, query: Arc<Query>) -> ResultStream<DocumentResult> {
        let matching_docs = match query.match_score(&self.filter) > 0 {
            true => query.matching_docs(&self.index, &self.filters),
            false => Vec::new(),
        };

        let futures = matching_docs
            .into_iter()
            .filter_map(|lcid|
                self.cids.get_by_left(&lcid).map(|cid| (lcid, cid.to_owned()))
            )
            .filter_map(|(lcid, cid)|
                self.filenames.get(&lcid).map(|path|
                    path.iter().filter_map(|(ldid, filename)| {
                        self.directories.get_by_left(ldid).map(|path| {
                            let mut path = path.clone();
                            path.push(filename.clone());
                            path
                        })
                    }).collect::<Vec<Vec<String>>>()
                ).map(|paths| (cid, paths))
            )
            .map(|(cid, paths)| cid_to_result_wrapper(Arc::clone(&query), cid, paths, Arc::clone(&self.config)))
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
            let mut paths: HashMap<String, Vec<Vec<String>>> = HashMap::new();
            let mut fetched_documents: HashSet<String> = self.documents().await;
            let mut prev_document_count = fetched_documents.len();
            while let Some(cid) = pinned.pop() {
                let parent_paths = paths.get(&cid).map(|p| p.to_owned()).unwrap_or_default();
                // FIXME: top level files are ignored later

                match ls(ipfs_rpc, cid, parent_paths).await {
                    Ok(mut new_links) => {
                        // Detect DNS-pins
                        if new_links.iter().all(|(_,p,_)| p.iter().any(|p| p.len() == 2 && p[1].starts_with("dns-pin-"))) {
                            // FIXME: handle malicious folders
                            for (_, new_paths, _) in &mut new_links {
                                let Some(name_pos) = new_paths.iter().position(|p| p.len() == 2 && p[1].starts_with("dns-pin-")) else {
                                    warn!("Invalid DNS pin: {paths:?}");
                                    continue;
                                };
                                let name = new_paths.remove(name_pos)[1][8..].to_owned();
                                let Some(i) = name.bytes().rposition(|b| b==b'-') else {
                                    warn!("Invalid DNS pin name: {name}");
                                    continue;
                                };
                                let (domain, _number) = name.split_at(i);
                                new_paths.push(vec![domain.to_owned()]);
                                trace!("Found DNS pin for {domain}");
                            }
                        }

                        for (child_cid, child_paths, child_is_file) in new_links {
                            if !child_is_file && !paths.contains_key(&child_cid) {
                                pinned.push(child_cid.clone());
                            }
                            if child_is_file && !fetched_documents.contains(&child_cid) && child_paths.iter().any(|p| p.last().map(|p| p.ends_with(".html")).unwrap_or(false)) {
                                let document = match fetch_document(ipfs_rpc, &child_cid).await {
                                    Ok(document) => document,
                                    Err(e) => {
                                        warn!("Error while fetching document: {e:?}");
                                        None
                                    },
                                };
                                fetched_documents.insert(child_cid.clone());
                                if let Some(document) = document {
                                    self.add_document(child_cid.clone(), document, child_paths.clone()).await;
                                }
                            }
                            
                            let old_child_paths = paths.entry(child_cid).or_default();
                            for path in child_paths {
                                if !old_child_paths.contains(&path) {
                                    old_child_paths.push(path);
                                }
                            }
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
            for (cid, paths) in paths {
                if !fetched_documents.contains(&cid) {
                    let document = match fetch_document(ipfs_rpc, &cid).await {
                        Ok(Some(document)) => document,
                        Ok(None) => continue,
                        Err(e) => {
                            warn!("Error while fetching document: {e:?}");
                            continue;
                        },
                    };
                    self.add_document(cid.clone(), document, paths).await;
                } else {
                    self.add_paths(&cid, paths).await;
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

    pub async fn add_document(&self, cid: String, document: Document, paths: Vec<Vec<String>>) {
        self.inner.write().await.add_document(cid, document, paths);
    }

    pub async fn add_paths(&self, cid: &String, paths: Vec<Vec<String>>) {
        self.inner.write().await.add_paths(cid, paths);
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
