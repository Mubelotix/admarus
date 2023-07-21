use crate::prelude::*;

pub struct RankedResults {
    pub results: HashMap<String, DocumentResult>,
    fully_ranked: Vec<(String, Scores)>,

    tf_ranking: Vec<(String, Score)>,
    variety_scores: HashMap<String, Score>,
    length_scores: HashMap<String, Score>,
    lang_scores: HashMap<String, Score>,

    providers: HashMap<String, HashSet<String>>,
    malicious_providers: HashSet<String>,
    verified: HashSet<String>,
}

impl RankedResults {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            fully_ranked: Vec::new(),
            tf_ranking: Vec::new(),
            variety_scores: HashMap::new(),
            length_scores: HashMap::new(),
            lang_scores: HashMap::new(),
            providers: HashMap::new(),
            malicious_providers: HashSet::new(),
            verified: HashSet::new(),
        }
    }

    pub fn insert(&mut self, mut res: DocumentResult, provider: String, query: &Query) {
        if let Some(previous_result) = self.results.get(&res.cid) {
            if !res.agrees_with(previous_result) {
                // TODO
                log!("Result {} from {} disagrees with previous result", res.cid, provider);
                return;
            }
        }

        res.rank_paths();
        self.providers.entry(res.cid.clone()).or_default().insert(provider);

        if self.results.contains_key(&res.cid) {
            return;
        }

        let tf_score = res.tf(query);
        let tf_rank = self.tf_ranking.binary_search_by_key(&tf_score, |(_,s)| *s).unwrap_or_else(|i| i);
        self.tf_ranking.insert(tf_rank, (res.cid.clone(), tf_score));

        self.variety_scores.insert(res.cid.clone(), res.variety_score(query));

        self.length_scores.insert(res.cid.clone(), res.length_score());

        self.lang_scores.insert(res.cid.clone(), res.lang_score(Lang::English));

        self.results.insert(res.cid.clone(), res);
    }

    pub fn rerank(&mut self) {
        let res_count = self.results.len() as f64;

        let mut tf_scores = HashMap::new();
        for (i, (cid, _)) in self.tf_ranking.iter().enumerate() {
            tf_scores.insert(cid, i as f64 / res_count);
        }

        let length_scores = &self.length_scores;

        let max_provider_count = self.providers.values().map(|v| v.len()).max().unwrap_or(0) as f64;
        self.fully_ranked = Vec::new();
        for (cid, _) in self.results.iter() {
            let Some(result) = self.results.get(cid) else {continue};
            let Some(providers) = self.providers.get(cid) else {continue};

            let Some(tf_score) = tf_scores.get(cid) else {continue};
            let Some(variety_score) = self.variety_scores.get(cid) else {continue};
            let Some(length_score) = length_scores.get(cid) else {continue};
            let Some(lang_score) = self.lang_scores.get(cid) else {continue};
            let popularity_score = Score::from(providers.len() as f64 / max_provider_count);
            let ipns_score = Score::from(result.has_ipns() as usize as f64);
            let verified_score = Score::from(self.verified.contains(cid) as usize as f64);

            let scores = Scores {
                tf_score: Score::from(*tf_score),
                variety_score: *variety_score,
                length_score: *length_score,
                lang_score: *lang_score,
                popularity_score,
                ipns_score,
                verified_score,
            };
            let i = self.fully_ranked.binary_search_by_key(&&scores, |(_,s)| s).unwrap_or_else(|i| i);
            self.fully_ranked.insert(i, (cid.clone(), scores));
        }
    } 

    pub fn verify_some(&mut self, top: usize, search_id: u64, ctx: &Context<ResultsPage>) {
        let rpc_addr = ctx.props().conn_status.rpc_addr();
        for (cid, _) in self.fully_ranked.iter().rev().take(top) {
            if self.verified.contains(cid) {
                continue;
            }
            let Some(untrusted_result) = self.results.get(cid) else {continue};
            let link = ctx.link().clone();
            let cid = cid.clone();
            let untrusted_result = untrusted_result.clone();
            spawn_local(async move {
                let trusted_result = match get_result(rpc_addr, search_id, &cid).await {
                    Ok(Some(result)) => result,
                    Ok(None) => {
                        link.send_message(ResultsMessage::MaliciousResult(cid));
                        return;
                    }
                    Err(e) => {
                        log!("Error fetching result {}: {:?}", cid, e);
                        return;
                    },
                };
                match untrusted_result.agrees_with(&trusted_result) {
                    true => link.send_message(ResultsMessage::VerifiedResult(cid, Box::new(trusted_result))),
                    false => link.send_message(ResultsMessage::MaliciousResult(cid)),
                }
            });
        }
    }

    pub fn malicious_result(&mut self, cid: String) {
        self.results.remove(&cid);
        let malicious_providers = self.providers.remove(&cid).unwrap_or_default();
        self.malicious_providers.extend(malicious_providers);
        for providers in self.providers.values_mut() {
            providers.retain(|p| !self.malicious_providers.contains(p));
        }
        self.providers.retain(|_, v| !v.is_empty());
        self.results.retain(|cid, _| !self.providers.get(cid).map(|v| v.is_empty()).unwrap_or(true));
    }

    pub fn verified_result(&mut self, cid: String, mut result: DocumentResult) {
        self.verified.insert(cid.clone());
        let old_paths = self.results.get(&cid).map(|r| r.paths.clone()).unwrap_or_default();
        result.paths = old_paths;
        self.results.insert(cid, result);
    }

    pub fn get_ranked(&self) -> &[(String, Scores)] {
        self.fully_ranked.as_slice()
    }

    pub fn iter_with_scores(&self) -> impl Iterator<Item = (&DocumentResult, &Scores)> {
        self.get_ranked().iter().rev().filter_map(|(cid, scores)| self.results.get(cid).map(|result| (result, scores)))
    }
}
