use crate::prelude::*;

pub struct RankedResults {
    pub results: HashMap<String, DocumentResult>,
    tf_ranking: Vec<(String, Score)>,
    variety_scores: HashMap<String, Score>,
    length_scores: HashMap<String, Score>,
    lang_scores: HashMap<String, Score>,
    providers: HashMap<String, Vec<String>>,
}

impl RankedResults {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            tf_ranking: Vec::new(),
            variety_scores: HashMap::new(),
            length_scores: HashMap::new(),
            lang_scores: HashMap::new(),
            providers: HashMap::new(),
        }
    }

    pub fn insert(&mut self, mut res: DocumentResult, provider: String, query: &Query) {
        res.rank_paths();
        self.providers.entry(res.cid.clone()).or_default().push(provider);

        if self.results.contains_key(&res.cid) {
            return;
        }

        let tf_score = Score::from(res.tf(query));
        let tf_rank = self.tf_ranking.binary_search_by_key(&tf_score, |(_,s)| *s).unwrap_or_else(|i| i);
        self.tf_ranking.insert(tf_rank, (res.cid.clone(), tf_score));

        self.variety_scores.insert(res.cid.clone(), res.variety_score(query));

        self.length_scores.insert(res.cid.clone(), res.length_score());

        self.lang_scores.insert(res.cid.clone(), res.lang_score(Lang::English));

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
            let Some(result) = self.results.get(cid) else {continue};
            let Some(providers) = self.providers.get(cid) else {continue};

            let Some(tf_score) = tf_scores.get(cid) else {continue};
            let Some(variety_score) = self.variety_scores.get(cid) else {continue};
            let Some(length_score) = length_scores.get(cid) else {continue};
            let Some(lang_score) = self.lang_scores.get(cid) else {continue};
            let popularity_score = Score::from(providers.len() as f64 / max_provider_count);
            let ipns_score = Score::from(result.has_ipns() as usize as f64);

            let scores = Scores {
                tf_score: Score::from(*tf_score),
                variety_score: *variety_score,
                length_score: *length_score,
                lang_score: *lang_score,
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
        scores.into_iter().rev().filter_map(|(cid, scores)| self.results.get(&cid).map(|result| (result, scores)))
    }
}
