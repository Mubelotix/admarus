use crate::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heading {
    pub title: String,
    pub level: usize,
}

/// Used to count words but counts different types of words separately.
/// The sum of all fields is the total number of words.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WordCount {
    #[serde(default)]
    h1: usize,
    #[serde(default)]
    h2: usize,
    #[serde(default)]
    h3: usize,
    #[serde(default)]
    h4: usize,
    #[serde(default)]
    h5: usize,
    #[serde(default)]
    h6: usize,
    /// Content with high importance
    #[serde(default)]
    strong: usize,
    /// Content with some emphasis
    #[serde(default)]
    em: usize,
    /// Normal text
    #[serde(default)]
    regular: usize,
    /// Content with low importance
    #[serde(default)]
    small: usize,
    /// No longer accurate or no longer relevant
    #[serde(default)]
    s: usize,
}

impl WordCount {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add(&mut self, h1: bool, h2: bool, h3: bool, h4: bool, h5: bool, h6: bool, strong: bool, em: bool, small: bool, s: bool) {
        if h1 { self.h1 += 1; return }
        if h2 { self.h2 += 1; return }
        if h3 { self.h3 += 1; return }
        if h4 { self.h4 += 1; return }
        if h5 { self.h5 += 1; return }
        if h6 { self.h6 += 1; return }
        if strong { self.strong += 1; return }
        if em { self.em += 1; return }
        if small { self.small += 1; return }
        if s { self.s += 1; return }
        self.regular += 1;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentResult {
    pub cid: String,
    pub paths: Vec<Vec<String>>,
    pub icon_cid: Option<String>,
    pub domain: Option<String>,
    pub title: String,
    pub description: String,

    /// Each query term is mapped to the number of times it appears in the document.
    /// Along with `word_count`, this can be used to calculate the tf-idf score.
    pub term_counts: Vec<WordCount>,
    /// The number of words in the document.
    pub word_count: WordCount,
}

impl SearchResult for DocumentResult {
    type Cid = String;
    type ParsingError = serde_json::Error;

    fn cid(&self) -> Self::Cid {
        self.cid.clone()
    }

    fn into_bytes(self) -> Vec<u8> {
        serde_json::to_vec(&self).unwrap_or_default()
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::ParsingError> {
        serde_json::from_slice(bytes)
    }
}
