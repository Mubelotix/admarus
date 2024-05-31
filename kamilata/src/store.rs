use crate::prelude::*;
use async_trait::async_trait;

pub trait SearchResult: Sized + std::fmt::Debug {
    type Cid: std::hash::Hash + Eq + Send + Sync + std::fmt::Debug;
    type ParsingError: std::error::Error + Send;

    fn cid(&self) -> Self::Cid;
    fn into_bytes(self) -> Vec<u8>;
    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::ParsingError>;
}

pub type ResultStream<SearchResult> = Pin<Box<dyn futures::Stream<Item = SearchResult> + Send>>;
pub type ResultStreamBuilderFut<SearchResult> = Pin<Box<dyn Future<Output = ResultStream<SearchResult>> + Send>>;

/// This library lets you manage your documents the way you want.
/// This trait must be implemented on your document store.
#[async_trait]
pub trait Store<const N: usize>: Send + Sync + 'static {
    type Result: SearchResult + Send + Sync;
    type Query: SearchQuery<N>;
    
    /// Hash a word the way you like.
    /// You can return multiple hashes for a single input (it's the idea behind Bloom filters).
    /// 
    /// Must return at least one value.
    /// Must return values lower than `N*8` as they will be used as bit indices in filters.
    fn hash_word(word: &str) -> Vec<usize>;

    /// Return a filter that has been filled with the words of the documents.
    /// This function is intented to return a cached value as the filter should have been generated earlier.
    async fn get_filter(&self) -> Filter<N>; // TODO: use reference?

    /// Search among all documents and return those matching at least `min_matching` words.
    /// 
    /// The return type is a future to a stream of results.
    fn search(&self, query: Arc<Self::Query>) -> ResultStreamBuilderFut<Self::Result>;
}
