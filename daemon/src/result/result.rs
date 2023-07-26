use crate::prelude::*;

/// Data as found in link[rel=icon].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FaviconDescriptor {
    pub href: String,
    pub mime_type: String,
    pub sizes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentResult {
    pub cid: String,
    pub favicons: Vec<FaviconDescriptor>,
    /// Size limit: Sum of all segment sizes <= 10kB
    pub paths: Vec<Vec<String>>,
    /// Content of the title tag
    /// Size limit: 1kB
    pub title: Option<String>,
    /// Content of the h1 tag  
    /// Required if title is not present
    /// Size limit: 1kB
    pub h1: Option<String>,
    /// Content of the meta description tag
    /// Size limit: 5kB
    pub description: Option<String>,
    /// This is a piece of text from the document that the provider thinks is relevant to the query.
    /// It is arbitrarily selected.  
    /// Required if description is not present
    /// Size limit: 5kB
    pub extract: Option<String>,

    /// Each query term is mapped to the number of times it appears in the document.
    /// Along with `word_count`, this can be used to calculate the tf-idf score.
    pub term_counts: Vec<WordCount>,
    /// The number of words in the document.
    pub word_count: WordCount,

    /// Present if daemon supports the language of the document.
    /// Is intended to represent the share of words in the document that are common in that language.
    /// Words are counted in bytes so that this metric is relevant with unsupported languages whose words are not properly isolated by the daemon.
    pub common_words: Option<f64>,
}
