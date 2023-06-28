use std::cmp::Ordering;

use crate::prelude::*;

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
    pub fn sum(&self) -> usize {
        self.h1 + self.h2 + self.h3 + self.h4 + self.h5 + self.h6 + self.strong + self.em + self.regular + self.small + self.s
    }
    
    fn weighted_sum(&self) -> f64 {
        self.h1 as f64 * 10.0
            + self.h2 as f64 * 9.0
            + self.h3 as f64 * 8.0
            + self.h4 as f64 * 7.0
            + self.h5 as f64 * 6.0
            + self.h6 as f64 * 5.5
            + self.strong as f64 * 4.0
            + self.em as f64 * 1.1
            + self.regular as f64 * 1.0
            + self.small as f64 * 0.3
            + self.s as f64 * 0.1
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

impl DocumentResult {
    fn tf(&self) -> f64 {
        // todo: check returned data
        let word_count_sum = self.word_count.weighted_sum();
        let term_sum = self.term_counts.iter().map(|wc| wc.weighted_sum()).sum::<f64>();
        term_sum / word_count_sum
    }

    pub fn score(&self) -> Score {
        Score::from(self.tf() * 1.0)
    }
}

pub struct Score {
    val: f64,
}

impl From<f64> for Score {
    fn from(val: f64) -> Self {
        Self { val }
    }
}

impl PartialEq for Score {
    fn eq(&self, other: &Self) -> bool {
        self.val == other.val
    }
}

impl Eq for Score {}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.val.partial_cmp(&other.val)
    }
}

impl Ord for Score {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl std::fmt::Display for Score {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.val)
    }
}

impl std::fmt::Debug for Score {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.val)
    }
}
