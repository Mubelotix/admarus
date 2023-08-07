use super::*;

pub(super) struct DocumentIndexInner {
    config: Arc<Args>,
    pub(super) filter: Filter<FILTER_SIZE>,

    pub(super) cid_counter: u32,
    pub(super) ancestors: HashMap<LocalCid, HashMap<LocalCid, String>>,
    pub(super) folders: HashSet<LocalCid>,
    pub(super) cids: BiHashMap<LocalCid, String>,

    loaded_index: HashSet<String>,
    in_use_index: HashMap<String, usize>,
    in_memory_index: HashMap<String, HashMap<LocalCid, f32>>,
    // todo filters

    index_db: DbIndexController,
}

impl DocumentIndexInner {
    pub fn new(config: Arc<Args>) -> DocumentIndexInner {
        let db = open_database(&config.database_path);
        let index_db = DbIndexController::from(db);

        DocumentIndexInner {
            config,
            filter: Filter::new(),
            
            cid_counter: 0,
            ancestors: HashMap::new(),
            folders: HashSet::new(),
            cids: BiHashMap::new(),

            loaded_index: HashSet::new(),
            in_use_index: HashMap::new(),
            in_memory_index: HashMap::new(),

            index_db,
        }
    }

    // TODO: optimize
    async fn load_index(&mut self, word: String) {
        let new_data = match self.index_db.get(word.clone()).await {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to load index for word {word}: {e:?}");
                return;
            }
        };
        self.loaded_index.insert(word.clone());
        self.in_memory_index.entry(word).or_default().extend(new_data.into_iter().filter(|(lcid, _)| self.cids.contains_left(lcid)));
    }
    async fn unload_index(&mut self, word: String) {
        if !self.loaded_index.contains(&word) {
            self.load_index(word.clone()).await;
        }
        if self.in_use_index.get(&word).unwrap_or(&0) > &0 {
            return;
        }
        let data = match self.in_memory_index.remove(&word) {
            Some(data) => data,
            None => return, // Was unloaded in the meantime
        };
        if let Err(e) = self.index_db.put(word.clone(), data).await {
            error!("Failed to unload index for word {word}: {e:?}");
            // TODO handle error
        }
    }
    async fn unload_index_batch(&mut self, words: Vec<String>) {
        let mut items = Vec::new();
        for word in words {
            if !self.loaded_index.contains(&word) {
                self.load_index(word.clone()).await; // TODO optimie
            }
            if self.in_use_index.get(&word).unwrap_or(&0) > &0 {
                continue;
            }
            let data = match self.in_memory_index.remove(&word) {
                Some(data) => data,
                None => return, // Was unloaded in the meantime
            };
            items.push((word, data));
        }
        if let Err(e) = self.index_db.put_batch(items).await {
            error!("Failed to unload index for words: {e:?}");
            // TODO handle error
        }
    }

    // TODO: optimize
    pub(super) async fn sweep(&mut self) {
        let start = Instant::now();
        let to_unload = self.in_memory_index.keys().filter(|word| !self.in_use_index.contains_key(*word)).cloned().collect::<Vec<_>>();
        let count = to_unload.len();
        self.unload_index_batch(to_unload).await;
        if count > 0 {
            trace!("Sweeped {count} words from index in {}ms", start.elapsed().as_millis());
        }
    }

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

    pub fn add_document(&mut self, cid: &String, doc: DocumentInspectionReport) {
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
            let frequencies = self.in_memory_index.entry(word.clone()).or_default();
            *frequencies.entry(lcid).or_insert(0.) += 1. / word_count as f32;
            self.filter.add_word::<DocumentIndex>(&word);
        }
        
        // Index by filters
        /*for (key, value) in doc.filters {
            self.filters.entry((key.to_string(), value.clone())).or_default().push(lcid);
            self.filter.add_word::<DocumentIndex>(&format!("{key}={value}"));
        }*/
    }

    pub fn update_filter(&mut self) {
        warn!("Unimplemented function: update_filter");
    }

    pub async fn search(&mut self, query: Arc<Query>) -> ResultStream<DocumentResult> {
        for term in query.terms() {
            *self.in_use_index.entry(term.to_owned()).or_default() += 1;
            self.load_index(term.clone()).await;
        }
        
        let matching_docs = match query.match_score(&self.filter) > 0 {
            true => query.matching_docs(&self.in_memory_index, &HashMap::new()), // TODO
            false => Vec::new(),
        };

        for term in query.terms() {
            *self.in_use_index.entry(term.to_owned()).or_default() -= 1;
        }

        let futures = matching_docs
            .into_iter()
            .filter_map(|lcid| self.cids.get_by_left(&lcid))
            .map(|cid| (cid, self.build_path(cid).unwrap_or_default()))
            .map(|(cid, paths)| cid_to_result_wrapper(Arc::clone(&query), cid.to_owned(), paths, Arc::clone(&self.config)))
            .collect();

        Box::pin(DocumentResultStream { futures })
    }
}
