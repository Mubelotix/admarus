use yew::virtual_dom::{VList, VText, VTag};

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
}

impl DocumentResult {
    fn rank_paths(&mut self) {
        self.paths.sort_by(|a, b| b.first().map(|f| f.contains('.')).cmp(&a.first().map(|f| f.contains('.'))).then_with(|| a.len().cmp(&b.len())));
    }
    
    pub fn format_result_title(&self) -> String {
        match self.title {
            Some(ref title) => title.clone(),
            None => match self.h1 {
                Some(ref h1) => h1.clone(),
                None => match self.paths.first() {
                    Some(path) => path.last().unwrap_or(&self.cid).clone(),
                    None => self.cid.clone(),
                }
            }
        }
    }

    pub fn format_best_addr(&self) -> String {
        let mut best_addr = match self.paths.first() {
            Some(f) => f.as_slice(),
            None => return format!("ipfs://{}", self.cid),
        };

        if best_addr.last().map(|l| l == "index.html").unwrap_or(false) {
            best_addr = &best_addr[..best_addr.len() - 1];
        }

        match best_addr.first().map(|f| f.contains('.')).unwrap_or(false) {
            true => format!("ipns://{}", best_addr.join("/")),
            false => format!("ipfs://{}", best_addr.join("/")),
        }
    }

    pub fn view_desc(&self, query: &[String]) -> VList {
        // TODO: this is a copy of daemon code
        fn extract_score(extract: &str, query: &[String]) -> usize {
            let mut score = 0;
            let mut extract_words = extract.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 3).map(|w| w.to_lowercase()).collect::<Vec<_>>();
            if extract_words.is_empty() {
                return 0;
            }
            let first_word = extract_words.remove(0);
            if query.contains(&first_word) {
                score += 4;
            }
            for word in query {
                if extract_words.contains(word) {
                    score += 1;
                }
            }
            score
        }

        let desc = match (&self.description, &self.extract) {
            (Some(desc), Some(extract)) => {
                if extract_score(desc, query) >= extract_score(extract, query) {
                    desc
                } else {
                    extract
                }
            }
            (Some(desc), None) => desc,
            (None, Some(extract)) => extract,
            (None, None) => return VList::new(),
        };
        let mut i = 0;
        let mut added = 0;
        let mut vlist = VList::new();
        for part in desc.split_inclusive(|c: char| !c.is_ascii_alphanumeric()) {
            let part_len = part.len();
            let word = part.trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
            if word.len() >= 3 && query.contains(&word.to_lowercase()) {
                if i - added > 0 {
                    let unbolded_text = desc[added..i].to_string();
                    vlist.add_child(VText::new(unbolded_text).into());
                }
                let bolded_text = desc[i..i + word.len()].to_string();
                let mut b_el = VTag::new("b");
                b_el.add_child(VText::new(bolded_text).into());
                vlist.add_child(b_el.into());
                added = i + word.len();
            }
            i += part_len;
        }
        if i - added > 0 {
            let unbolded_text = desc[added..i].to_string();
            vlist.add_child(VText::new(unbolded_text).into());
        }

        vlist
    }
}

// Score computations
impl DocumentResult {
    fn tf(&self, query: &[String]) -> f64 {
        let word_count_sum = self.word_count.weighted_sum();
        let term_sum = self.term_counts.iter().map(|wc| wc.weighted_sum()).sum::<f64>();
        
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
        let title_term_count = title_words.iter().filter(|w| query.contains(w)).count();
        let title_term_sum = title_term_count as f64 * 12.0;
        let title_word_sum = title_word_count as f64 * 12.0;

        (term_sum + title_term_sum) / (word_count_sum + title_word_sum)
    }

    fn length_score(&self) -> Score {
        let preferred_lenght = 500.0;
        let length = self.word_count.sum() as f64;
        let mut length_score = 1.0 / (1.0 + (-0.017 * (length - (preferred_lenght / 2.0))).exp());
        if length_score >= 0.995 {
            length_score = 1.0;
        }

        Score::from(length_score)
    }

    fn has_ipns(&self) -> bool {
        self.paths.iter().any(|p| p.first().map(|f| f.contains('.')).unwrap_or(false))
    }

    fn lang_score(&self, requested_lang: Lang) -> Score {
        let mut words = Vec::new();
        let description_lowercase = self.description.as_ref().map(|d| d.to_lowercase());
        let extract_lowercase = self.extract.as_ref().map(|e| e.to_lowercase());
        if let Some(description) = &description_lowercase {
            words.extend(description.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 3));
        }
        if let Some(extract) = &extract_lowercase {
            words.extend(extract.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 3));
        }

        let word_count = words.len();
        let lang_words = requested_lang.common_words();
        let in_lang_count = words.iter().filter(|w| lang_words.sorted_contains(w)).count();
        let ratio = in_lang_count as f64 / word_count as f64;

        let mut score = ratio * 3.0;
        if score > 1.0 {
            score = 1.0;
        }

        Score::from(score)
    }
}

pub struct RankedResults {
    pub results: HashMap<String, DocumentResult>,
    tf_ranking: Vec<(String, Score)>,
    length_scores: HashMap<String, Score>,
    providers: HashMap<String, Vec<String>>,
}

impl RankedResults {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            tf_ranking: Vec::new(),
            length_scores: HashMap::new(),
            providers: HashMap::new(),
        }
    }

    pub fn insert(&mut self, mut res: DocumentResult, provider: String, query: &[String]) {
        res.rank_paths();
        self.providers.entry(res.cid.clone()).or_default().push(provider);

        if self.results.contains_key(&res.cid) {
            return;
        }

        let tf_score = Score::from(res.tf(query));
        let tf_rank = self.tf_ranking.binary_search_by_key(&tf_score, |(_,s)| *s).unwrap_or_else(|i| i);
        self.tf_ranking.insert(tf_rank, (res.cid.clone(), tf_score));

        self.length_scores.insert(res.cid.clone(), res.length_score());

        self.results.insert(res.cid.clone(), res);
    }

    pub fn get_all_scores(&self) -> Vec<(String, Scores)> {
        let res_count = self.results.len() as f64;

        let mut tf_scores = HashMap::new();
        for (i, (cid, _)) in self.tf_ranking.iter().enumerate() {
            tf_scores.insert(cid, i as f64 / res_count);
        }

        let length_scores = &self.length_scores;

        let max_provider_count = self.providers.values().map(|v| v.len()).max().unwrap_or(0) as f64;
        let mut all_scores = Vec::new();
        for (cid, _) in self.results.iter() {
            let tf_score = tf_scores.get(cid).unwrap();
            let length_score = length_scores.get(cid).unwrap();
            let popularity_score = Score::from(self.providers.get(cid).unwrap().len() as f64 / max_provider_count);
            let ipns_score = Score::from(self.results.get(cid).unwrap().has_ipns() as usize as f64);
            let scores = Scores {
                tf_score: Score::from(*tf_score),
                length_score: *length_score,
                popularity_score,
                ipns_score,
            };
            let i = all_scores.binary_search_by_key(&&scores, |(_,s)| s).unwrap_or_else(|i| i);
            all_scores.insert(i, (cid.clone(), scores));
        }

        all_scores
    }

    pub fn iter_with_scores(&self) -> impl Iterator<Item = (&DocumentResult, Scores)> {
        let scores = self.get_all_scores();
        scores.into_iter().rev().map(move |(cid, scores)| (self.results.get(&cid).unwrap(), scores))
    }
}
