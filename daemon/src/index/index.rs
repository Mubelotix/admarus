use super::*;

use tantivy::aggregation::agg_req::Aggregations;
use tantivy::aggregation::agg_result::AggregationResults;
use tantivy::aggregation::AggregationCollector;
use tantivy::query::AllQuery;
use tantivy::schema::{self, Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, FAST, STORED, STRING, TEXT};
use tantivy::{doc, DocId, Index, IndexReader, IndexWriter, Opstamp, TantivyDocument, TantivyError};
use tokio::sync::RwLockReadGuard;

fn build_schema() -> (Schema, Field, Field, Field) {
    let STRING_NOT_INDEXED = TextOptions::default();

    let mut schema_builder = Schema::builder();
    let cid_field = schema_builder.add_text_field("cid", STRING_NOT_INDEXED | STORED);
    // schema_builder.add_text_field("titles", TEXT | STORED);
    let desc_field = schema_builder.add_text_field("description", TEXT | STORED);
    let content_field = schema_builder.add_text_field("content", TEXT);
    // schema_builder.add_json_field("structured_data", TEXT | STORED);

    (schema_builder.build(), cid_field, desc_field, content_field)
}

#[derive(Clone)]
pub struct DocumentIndex {
    config: Arc<Args>,
    index: Index,
    status: Arc<RwLock<IndexingStatus>>,
    inner: Arc<RwLock<DocumentIndexInner>>,
    writer: Arc<RwLock<IndexWriter>>
}

#[allow(dead_code)]
impl DocumentIndex {
    pub async fn new(config: Arc<Args>) -> DocumentIndex {
        let (schema, cid_field, desc_field, content_field) = build_schema();
        let index = Index::create_in_ram(schema);
        let index_writer: IndexWriter = index.writer(50_000_000).expect("Couldn't build index writer"); // TODO: add a config option for index memory budget

        DocumentIndex {
            inner: Arc::new(RwLock::new(DocumentIndexInner::new(Arc::clone(&config), cid_field, desc_field, content_field).await)),
            status: Arc::new(RwLock::new(IndexingStatus::default())),
            config,
            index,
            writer: Arc::new(RwLock::new(index_writer))
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
            let mut to_load_unprioritized = HashMap::new();
            
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
            self.set_status(listed.len(), to_list.len(), loaded.len(), to_load.len(), to_load_unprioritized.len()).await;

            // Explore directories
            let start = Instant::now();
            if !to_list.is_empty() {debug!("{} elements to list", to_list.len())}
            while let Some(cid) = to_list.pop() {
                if !listed.insert(cid.clone()) {continue}
                self.set_status(listed.len(), to_list.len()+1, loaded.len(), to_load.len(), to_load_unprioritized.len()).await;

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
                        self.add_ancestor(&child_cid, child_name, child_is_folder, &cid).await;
                        if !listed.contains(&child_cid) {
                            to_list.push(child_cid);
                        }
                    } else if !loaded.contains(&child_cid) {
                        match child_name.ends_with(".html") {
                            true => to_load.insert(child_cid, (child_name, cid.clone())),
                            false => to_load_unprioritized.insert(child_cid, (child_name, cid.clone())),
                        };
                    }
                }
                to_list.sort();
                to_list.dedup();
            }

            // Load documents
            to_load_unprioritized.retain(|cid, _| !to_load.contains_key(cid));
            if !to_load.is_empty() {debug!("{} documents to load ({:.02?}s)", to_load.len(), start.elapsed().as_secs_f32())}
            let (to_load_len, to_load_unprioritized_len) = (to_load.len(), to_load_unprioritized.len());
            for (i, (cid, (name, parent_cid))) in to_load.drain().chain(to_load_unprioritized.drain()).enumerate() {
                let remaining_to_load = to_load_len.saturating_sub(i);
                let remaining_unprioritized = std::cmp::min(to_load_unprioritized_len, to_load_len + to_load_unprioritized_len - i);
                self.set_status(listed.len(), to_list.len(), loaded.len(), remaining_to_load, remaining_unprioritized).await;

                loaded.insert(cid.clone());
                let Ok(document) = fetch_document(ipfs_rpc, &cid).await else {continue};
                let Some(inspected) = inspect_document(document) else {continue};
                self.add_document(cid.to_owned(), inspected).await;
                self.add_ancestor(&cid, name, false, &parent_cid).await;
            }
            
            // Update filter
            self.set_status(listed.len(), 0, loaded.len(), 0, 0).await;
            self.set_status_updating_filter(true).await;
            self.update_filter().await;
            self.set_status_updating_filter(false).await;
            let load = self.get_filter().await.load()*100.0;
            if load != previous_load {
                previous_load = load;
                debug!("Filter filled at {load:.04}% ({:02}s)", start.elapsed().as_secs_f32());
            }
            sleep(Duration::from_secs(REFRESH_INTERVAL)).await;
        }
    }

    async fn set_status(&self, listed: usize, to_list: usize, loaded: usize, to_load: usize, to_load_unprioritized: usize) {
        let mut status = self.status.write().await;
        status.listed = listed;
        status.to_list = to_list;
        status.loaded = loaded;
        status.to_load = to_load;
        status.to_load_unprioritized = to_load_unprioritized;
    }

    async fn set_status_updating_filter(&self, updating_filter: bool) {
        let mut status = self.status.write().await;
        status.updating_filter = updating_filter;
    }

    pub async fn status(&self) -> IndexingStatus {
        self.status.read().await.clone()
    }

    pub async fn documents(&self) -> HashSet<String> {
        self.inner.read().await.documents()
    }

    pub async fn document_count(&self) -> usize {
        self.inner.read().await.document_count()
    }

    pub async fn add_document(&self, cid: String, doc: DocumentInspectionReport) -> Result<Opstamp, TantivyError> {
        let mut inner = self.inner.write().await;
        let writer = self.writer.read().await;
        inner.add_document(writer, cid, doc)
    }

    pub async fn add_ancestor(&self, cid: &String, name: String, is_folder: bool, folder_cid: &String) {
        self.inner.write().await.add_ancestor(cid, name, is_folder, folder_cid);
    }

    pub async fn build_path(&self, cid: &String) -> Option<Vec<Vec<String>>> {
        self.inner.read().await.build_path(cid)
    }

    pub async fn update_filter(&self) -> Result<(), TantivyError> {
        let mut reader = self.index.reader()?;
        self.inner.write().await.update_filter(reader).await
    }
}

pub(super) struct DocumentIndexInner {
    config: Arc<Args>,
    cid_field: Field,
    desc_field: Field,
    content_field: Field,

    pub(super) filter: Filter<FILTER_SIZE>,
    filter_needs_update: bool,

    pub(super) ancestors: HashMap<String, HashMap<String, String>>,
    pub(super) folders: HashSet<String>,
    pub(super) doc_ids: HashMap<String, DocId>,
}

impl DocumentIndexInner {
    pub async fn new(config: Arc<Args>, cid_field: Field, desc_field: Field, content_field: Field) -> DocumentIndexInner {
        DocumentIndexInner {
            config,
            cid_field,
            desc_field,
            content_field,

            filter: Filter::new(),
            filter_needs_update: false,

            ancestors: HashMap::new(),
            folders: HashSet::new(),
            doc_ids: HashMap::new(),
        }
    }   
    
    #[allow(dead_code)]
    pub(super) async fn sweep(&mut self) {}

    pub fn documents(&self) -> HashSet<String> {
        self.doc_ids
            .keys()
            .map(|cid| cid.to_owned())
            .collect()
    }

    pub fn document_count(&self) -> usize {
        self.doc_ids.len()
    }

    pub fn add_document(&mut self, index_writer: RwLockReadGuard<'_, IndexWriter>, cid: String, doc: DocumentInspectionReport) -> Result<Opstamp, TantivyError> {
        let opstamp = index_writer.add_document(doc!(
            self.cid_field => cid,
            self.desc_field => doc.description.unwrap_or_default(),
            self.content_field => doc.text_content,
        ))?;

        // TODO: tell we need to update our filter after commit

        Ok(opstamp)
    }

    pub async fn update_filter(&mut self, index_reader: IndexReader) -> Result<(), TantivyError> {
        if !self.filter_needs_update {
            return Ok(());
        }
        let searcher = index_reader.searcher();
        self.filter = Filter::new();
        for segment_reader in searcher.segment_readers() {
            let inverted_index = segment_reader.inverted_index(self.content_field)?; // TODO: also read title and description
            let terms = inverted_index.terms();
            let mut term_stream = terms.stream().unwrap();

            while let Some((u, v)) = term_stream.next() {
                self.filter.add_bytes::<DocumentIndex>(u);
            }
        }
        self.filter_needs_update = false;
        Ok(())
    }

    // TODO: switching self to static may improve performance by a lot
    pub async fn search(&self, query: Arc<Query>) -> ResultStream<DocumentResult> {
        let matching_docs = match query.match_score(&self.filter) > 0 {
            true => query.matching_docs(&self.index, &HashMap::new()), // Restore filters
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

    pub fn add_ancestor(&mut self, cid: &String, name: String, is_folder: bool, folder_cid: &String) {
        let lcid = match self.cids.get_by_right(cid) {
            Some(lcid) => lcid.to_owned(),
            None => {
                let lcid = LocalCid(self.cid_counter);
                self.cid_counter += 1;
                self.cids.insert(lcid, cid.clone());
                lcid
            }
        };
        if is_folder {
            self.folders.insert(lcid);
        }

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
                    ancestor.clone_into(&mut new_path.0);
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
}

#[async_trait]
impl Store<FILTER_SIZE> for DocumentIndex {
    type Result = DocumentResult;
    type Query = Query;

    fn hash_bytes(word: &[u8]) -> Vec<usize>  {
        let mut result = 1usize;
        const RANDOM_SEED: [usize; 16] = [542587211452, 5242354514, 245421154, 4534542154, 542866467, 545245414, 7867569786914, 88797854597, 24542187316, 645785447, 434963879, 4234274, 55418648642, 69454242114688, 74539841, 454214578213];
        for c in word {
            for i in 0..8 {
                result = result.overflowing_mul(*c as usize + RANDOM_SEED[i*2]).0;
                result = result.overflowing_add(*c as usize + RANDOM_SEED[i*2+1]).0;
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
