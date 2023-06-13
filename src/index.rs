use std::{sync::Arc, collections::HashSet};
use crate::prelude::*;

const REFRESH_PINNED_INTERVAL: u64 = 120;

struct DocumentIndexInner<const N: usize> {
    config: Arc<Args>,
    pub filter: Filter<N>,
    filter_needs_update: bool,

    metadata: HashMap<String, Metadata>,

    /// word -> [cid -> frequency]
    pub index: HashMap<String, HashMap<String, f64>>, // FIXME: no field should be public
}

impl<const N: usize> DocumentIndexInner<N> {
    pub fn new(config: Arc<Args>) -> DocumentIndexInner<N> {
        DocumentIndexInner {
            config,
            filter: Filter::new(),
            filter_needs_update: false,
            metadata: HashMap::new(),
            index: HashMap::new(),
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
        self.metadata.remove(cid);
        for frequencies in self.index.values_mut() {
            frequencies.remove(cid);
        }
        let previous_len = self.index.len();
        self.index.retain(|_, frequencies| !frequencies.is_empty());
        if previous_len != self.index.len() {
            self.filter_needs_update = true;
        }
    }

    pub fn add_document(&mut self, cid: String, document: Document, metadata: Metadata) {
        self.metadata.insert(cid.clone(), metadata);
        let word_count = document.words().count() as f64;
        for word in document.words() {
            let frequencies = self.index.entry(word.clone()).or_insert_with(HashMap::new);
            *frequencies.entry(cid.clone()).or_insert(0.) += 1. / word_count;
            self.filter.add_word::<DocumentIndex<N>>(word);
        }
    }

    pub fn add_documents(&mut self, documents: Vec<(String, Document, Metadata)>) {
        for (cid, document, link) in documents {
            self.add_document(cid, document, link);
        }
    }

    pub async fn search(&self, words: Vec<String>, min_matching: usize) -> Vec<DocumentResult> {
        if words.iter().filter(|w| self.filter.get_word::<DocumentIndex<N>>(w)).count() < min_matching {
            return Vec::new();
        }

        let mut matching_cids = HashMap::new();
        for word in words {
            for (document, _freqency) in self.index.get(&word).into_iter().flatten() {
                *matching_cids.entry(document.to_owned()).or_insert(0) += 1;
            }
        }
        matching_cids.retain(|_,c| *c>=min_matching);

        let mut results = Vec::new();
        for (cid, _) in matching_cids {
            let Ok(Some(document)) = fetch_document(&self.config.ipfs_rpc, &cid).await else {continue};
            let Some(metadata) = self.metadata.get(&cid) else {continue};
            results.push(document.into_result(cid, metadata.to_owned()));
        }
        results
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
            
            let pinned_files = explore_all(&self.config.ipfs_rpc, pinned).await;
            let documents = collect_documents(&self.config.ipfs_rpc, pinned_files).await;
            debug!("{} new documents", documents.len());

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
    type SearchResult = DocumentResult;

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

    fn search(&self, words: Vec<String>, min_matching: usize) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<Self::SearchResult> > +Send+Sync+'static> >  {
        let inner2 = Arc::clone(&self.inner);
        Box::pin(async move {
            inner2.read().await.search(words, min_matching).await
        })
    }
}
