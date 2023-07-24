use crate::prelude::*;

impl Query {
    fn map_count(&self, counts: &HashMap<&String, f64>) -> f64 {
        self.root.clone_only_words().map(|r| r.map_counts(counts)).unwrap_or(1.0)
    }
}

impl QueryComp {
    #[track_caller]
    fn map_counts(&self, counts: &HashMap<&String, f64>) -> f64 {
        match self {
            QueryComp::Word(w) => counts.get(w).copied().unwrap_or(0.0),
            QueryComp::Filter { .. } => panic!("QueryComp::map_counts() called on filter"),
            QueryComp::Not(_) => panic!("QueryComp::map_counts() called on not"),
            QueryComp::NAmong { n, among } => {
                let mut mapped_counts = among.iter().map(|c| c.map_counts(counts)).collect::<Vec<_>>();
                mapped_counts.sort_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));
                mapped_counts.into_iter().take(*n).sum::<f64>() / *n as f64
            }
        }
    }
}

impl DocumentResult {
    pub fn tf(&self, query: &Query) -> Score {
        let query_terms = query.positive_terms();
        let word_count = self.word_count.weighted_sum();
        let mut counts = HashMap::new();
        for (term_count, term) in self.term_counts.iter().zip(&query_terms) {
            counts.insert(*term, term_count.weighted_sum());
        }
        let term_count = query.map_count(&counts);
        
        // Title is counted separately as it is not part of the document body
        let title_words  = self.title
            .as_ref()
            .map(|t| t
                .to_lowercase()
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|w| w.len() >= 3)
                .map(|w| w.to_string())
            .collect::<Vec<_>>())
            .unwrap_or_default();
        let title_word_count = title_words.len();
        let title_word_count = title_word_count as f64 * 12.0;
        let mut counts = HashMap::new();
        for word in &title_words {
            counts.insert(word, 12.0);
        }
        let title_term_count = query.map_count(&counts);

        Score::from((term_count + title_term_count) / (word_count + title_word_count))
    }

    pub fn variety_score(&self, query: &Query) -> Score {
        let mut title_words: Option<Vec<String>> = None;
        
        let mut score = 0.0;
        for (i, (term, weight)) in query.weighted_terms().into_iter().enumerate() {
            if self.term_counts[i].sum() > 0 {
                score += weight;
            } else {
                let title_words_some = match title_words.take() {
                    Some(title_words) => title_words,
                    None => {
                        let Some(title_words_some) = self.title
                            .as_ref()
                            .map(|t| t
                                .to_lowercase()
                                .split(|c: char| !c.is_ascii_alphanumeric())
                                .filter(|w| w.len() >= 3)
                                .map(|w| w.to_string())
                            .collect::<Vec<_>>())
                            else {continue};
                        title_words_some
                    }
                };
                if title_words_some.contains(&term) {
                    score += weight;
                }
                title_words = Some(title_words_some);
            }
        }

        Score::from(score)
    }

    pub fn length_score(&self) -> Score {
        let preferred_lenght = 500.0;
        let length = self.word_count.sum() as f64;
        let mut length_score = 1.0 / (1.0 + (-0.017 * (length - (preferred_lenght / 2.0))).exp());
        if length_score >= 0.995 {
            length_score = 1.0;
        }

        Score::from(length_score)
    }

    pub fn has_ipns(&self) -> bool {
        self.paths.iter().any(|p| p.first().map(|f| f.contains('.')).unwrap_or(false))
    }

    pub fn lang_score(&self, requested_lang: Lang) -> Score {
        let common_words = match self.common_words {
            Some(common_words) => common_words,
            None => {
                let mut words = Vec::new();
                let description_lowercase = self.description.as_ref().map(|d| d.to_lowercase());
                let extract_lowercase = self.extract.as_ref().map(|e| e.to_lowercase());
                if let Some(description) = &description_lowercase {
                    words.extend(description.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 3));
                }
                if let Some(extract) = &extract_lowercase {
                    words.extend(extract.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 3));
                }

                let words_bytes = words.iter().map(|w| w.len()).sum::<usize>();
                let lang_words = requested_lang.common_words();
                let common_words_bytes = words.iter().filter(|w| lang_words.sorted_contains(w)).map(|w| w.len()).sum::<usize>();
                common_words_bytes as f64 / words_bytes as f64
            },
        };

        let mut score = common_words * 2.0;
        if score > 1.0 {
            score = 1.0;
        }

        Score::from(score)
    }
}

#[derive(Clone, Copy)]
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
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
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

#[derive(Clone)]
pub struct Scores {
    pub tf_score: Score,
    pub variety_score: Score,
    pub length_score: Score,
    pub lang_score: Score,
    pub popularity_score: Score,
    pub ipns_score: Score,
    pub verified_score: Score,
}

impl Scores {
    /// This computes the final score for a document.
    pub fn general_score(&self) -> Score {
        Score::from(
            (self.popularity_score.val * 0.49
            + self.variety_score.val * 0.2
            + self.tf_score.val * 0.2
            + self.ipns_score.val * 0.10
            + self.verified_score.val * 0.01)
            
            // Scores that multiply are those we want to always be 1.0
            * self.lang_score.val
            * self.length_score.val
        )
    }

    pub fn zero() -> Self {
        Self {
            tf_score: Score::from(0.0),
            variety_score: Score::from(0.0),
            length_score: Score::from(0.0),
            lang_score: Score::from(0.0),
            popularity_score: Score::from(0.0),
            ipns_score: Score::from(0.0),
            verified_score: Score::from(0.0),
        }
    }
}

impl PartialEq for Scores {
    fn eq(&self, other: &Self) -> bool {
        self.tf_score == other.tf_score && self.length_score == other.length_score
    }
}

impl Eq for Scores {}

impl PartialOrd for Scores {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.general_score().partial_cmp(&self.general_score())
    }
}

impl Ord for Scores {
    fn cmp(&self, other: &Self) -> Ordering {
        other.general_score().partial_cmp(&self.general_score()).unwrap_or(Ordering::Equal)
    }
}
