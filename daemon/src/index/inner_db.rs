use super::*;

pub(super) struct DocumentIndexInner {
    config: Arc<Args>,
    pub(super) filter: Filter<FILTER_SIZE>,

    cid_counter: u32,
    ancestors: HashMap<LocalCid, HashMap<LocalCid, String>>,
    folders: HashSet<LocalCid>,
    cids: BiHashMap<LocalCid, String>,

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

    // TODO: move outside inner
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
        let data = match self.in_memory_index.remove(&word) {
            Some(data) => data,
            None => return, // Was unloaded in the meantime
        };
        self.index_db.put(word, data).await;
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
