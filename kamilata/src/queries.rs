use crate::prelude::*;

/// A trait that represent your search query.
/// It might contain fancy features like OR and NOT, and even categories (like language).
pub trait SearchQuery<const N: usize>: Clone + Sized + Send + Sync {
    type ParsingError: std::error::Error;

    /// This is used to test if a filter matches the query.
    /// It should return a score, the higher the better.
    /// Returning 0 means the filter doesn't match.
    /// 
    /// Note: If you are just getting started, you can just return 1 if the filter matches, and 0 otherwise.
    fn match_score(&self, filter: &Filter<N>) -> u32;

    fn to_bytes(&self) -> Vec<u8>;
    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::ParsingError>;
}
