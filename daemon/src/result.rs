use crate::prelude::*;

/// Used to count words but counts different types of words separately.
/// The sum of all fields is the total number of words.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WordCount {
    /// Counters for: h1, h2, h3, h4, h5, h6, strong, em, regular, small, s
    data: [usize; 11]
}

impl WordCount {
    pub const fn h1(&self) -> usize {
        self.data[0]
    }

    pub const fn h2(&self) -> usize {
        self.data[1]
    }

    pub const fn h3(&self) -> usize {
        self.data[2]
    }

    pub const fn h4(&self) -> usize {
        self.data[3]
    }

    pub const fn h5(&self) -> usize {
        self.data[4]
    }

    pub const fn h6(&self) -> usize {
        self.data[5]
    }

    pub const fn strong(&self) -> usize {
        self.data[6]
    }

    pub const fn em(&self) -> usize {
        self.data[7]
    }

    pub const fn regular(&self) -> usize {
        self.data[8]
    }

    pub const fn small(&self) -> usize {
        self.data[9]
    }

    pub const fn s(&self) -> usize {
        self.data[10]
    }

    pub fn sum(&self) -> usize {
        self.data.iter().sum()
    }
    
    pub fn weighted_sum(&self) -> f64 {
        self.h1() as f64 * 10.0
            + self.h2() as f64 * 9.0
            + self.h3() as f64 * 8.0
            + self.h4() as f64 * 7.0
            + self.h5() as f64 * 6.0
            + self.h6() as f64 * 5.5
            + self.strong() as f64 * 4.0
            + self.em() as f64 * 1.1
            + self.regular() as f64 * 1.0
            + self.small() as f64 * 0.3
            + self.s() as f64 * 0.1
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add(&mut self, h1: bool, h2: bool, h3: bool, h4: bool, h5: bool, h6: bool, strong: bool, em: bool, small: bool, s: bool) {
        if h1 { self.data[0] += 1; return }
        if h2 { self.data[1] += 1; return }
        if h3 { self.data[2] += 1; return }
        if h4 { self.data[3] += 1; return }
        if h5 { self.data[4] += 1; return }
        if h6 { self.data[5] += 1; return }
        if strong { self.data[6] += 1; return }
        if em { self.data[7] += 1; return }
        if small { self.data[9] += 1; return }
        if s { self.data[10] += 1; return }
        self.data[8] += 1;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentResult {
    pub cid: String,
    pub paths: Vec<Vec<String>>,
    pub icon_cid: Option<String>,
    pub domain: Option<String>,
    /// Content of the title tag
    pub title: Option<String>,
    /// Content of the h1 tag  
    /// Required if title is not present
    pub h1: Option<String>,
    /// Content of the meta description tag
    pub description: Option<String>,
    /// This is a piece of text from the document that the provider thinks is relevant to the query.
    /// It is arbitrarily selected.  
    /// Required if description is not present
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
