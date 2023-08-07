use super::*;

pub(super) struct DocumentIndexInner {
    config: Arc<Args>,

    pub(super) filter: Filter<FILTER_SIZE>,
    filter_needs_update: bool,

    pub(super) cid_counter: u32,
    pub(super) ancestors: HashMap<LocalCid, HashMap<LocalCid, String>>,
    pub(super) folders: HashSet<LocalCid>,
    pub(super) cids: BiHashMap<LocalCid, String>,

    index: HashMap<String, HashMap<LocalCid, f32>>,
    filters: HashMap<(String, String), Vec<LocalCid>>,
}

impl DocumentIndexInner {
    pub fn new(config: Arc<Args>) -> DocumentIndexInner {
        DocumentIndexInner {
            config,
            filter: Filter::new(),
            filter_needs_update: false,

            ancestors: HashMap::new(),
            folders: HashSet::new(),

            cids: BiHashMap::new(),
            cid_counter: 0,

            index: HashMap::new(),
            filters: HashMap::new()
        }
    }   
    
    pub(super) async fn sweep(&mut self) {}

    pub fn folders(&self) -> HashMap<String, usize> {
        let mut folders = HashMap::new();
        for lcid in self.cids.left_values() {
            let Some(ancestor_lcid) = self.ancestors.get(lcid).and_then(|a| a.keys().next()) else {continue}; // TODO: files not in folder
            let Some(ancestor_cid) = self.cids.get_by_left(ancestor_lcid) else {continue};
            *folders.entry(ancestor_cid.to_owned()).or_default() += 1;
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
            self.filter.add_word::<DocumentIndex>(word);
        }
        self.filter_needs_update = false;
    }

    pub async fn add_document(&mut self, cid: &String, doc: DocumentInspectionReport) {
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
        let word_count = doc.words.len() as f64;
        for word in doc.words {
            let frequencies = self.index.entry(word.clone()).or_default();
            *frequencies.entry(lcid).or_insert(0.) += 1. / word_count as f32;
            self.filter.add_word::<DocumentIndex>(&word);
        }
        
        // Index by filters
        for (key, value) in doc.filters {
            self.filters.entry((key.to_string(), value.clone())).or_default().push(lcid);
            self.filter.add_word::<DocumentIndex>(&format!("{key}={value}"));
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
            .filter_map(|lcid| self.cids.get_by_left(&lcid))
            .map(|cid| (cid, self.build_path(cid).unwrap_or_default()))
            .map(|(cid, paths)| cid_to_result_wrapper(Arc::clone(&query), cid.to_owned(), paths, Arc::clone(&self.config)))
            .collect();

        Box::pin(DocumentResultStream { futures })
    }
}
