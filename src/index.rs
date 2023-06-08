use crate::prelude::*;

pub struct DocumentIndex<const N: usize> {
    pub filter: Filter<N>,
    filter_needs_update: bool,

    /// cid -> document
    documents: HashMap<String, Document>,

    /// word -> [cid -> frequency]
    pub index: HashMap<String, HashMap<String, f64>>, // FIXME: no field should be public
}

impl<const N: usize> DocumentIndex<N> {
    pub fn new() -> DocumentIndex<N> {
        DocumentIndex {
            filter: Filter::new(),
            filter_needs_update: false,
            documents: HashMap::new(),
            index: HashMap::new(),
        }
    }

    pub fn update_filter(&mut self) {
        if !self.filter_needs_update {
            return;
        }
        self.filter = Filter::new();
        for word in self.index.keys() {
            self.filter.add_word::<Self>(word);
        }
        self.filter_needs_update = false;
    }

    pub fn remove_document(&mut self, cid: &str) {
        self.documents.remove(cid);
        for frequencies in self.index.values_mut() {
            frequencies.remove(cid);
        }
        let previous_len = self.index.len();
        self.index.retain(|_, frequencies| !frequencies.is_empty());
        if previous_len != self.index.len() {
            self.filter_needs_update = true;
        }
    }

    pub fn add_document(&mut self, document: Document) {
        let word_count = document.words().count() as f64;
        for word in document.words() {
            let frequencies = self.index.entry(word.clone()).or_insert_with(HashMap::new);
            *frequencies.entry(document.link.cid.clone()).or_insert(0.) += 1. / word_count;
            self.filter.add_word::<Self>(word);
        }
        self.documents.insert(document.link.cid.clone(), document);
    }

    pub fn add_documents(&mut self, documents: Vec<Document>) {
        for document in documents {
            self.add_document(document);
        }
    }
}

#[async_trait]
impl<const N: usize> Store<N> for DocumentIndex<N> {
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
        self.filter.clone()
    }

    fn search(&self, words:Vec<String>, min_matching:usize) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<Self::SearchResult> > +Send+Sync+'static> >  {
        todo!()
    }
}
