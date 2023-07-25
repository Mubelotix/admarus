use crate::prelude::*;

pub struct RankedResults {
    pub results: HashMap<String, DocumentResult>,
    /// Grouping results are results whose title directly matches the query.
    /// Other results under the same path are grouped under the grouping result.
    grouping_results: HashSet<String>,
    fully_ranked: Vec<GroupedResults>,

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
            grouping_results: HashSet::new(),
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

        if res.is_grouping_result(query) {
            // FIXME: handle the case where a grouping result is itself grouped under another grouping result
            self.grouping_results.insert(res.cid.clone());
        }
        self.results.insert(res.cid.clone(), res);
    }

    fn get_scores(&self, cid: &String, tf_score: Score) -> Option<Scores> {
        let max_provider_count = self.providers.values().map(|v| v.len()).max().unwrap_or(0) as f64;

        let Some(result) = self.results.get(cid) else {return None};
        let Some(providers) = self.providers.get(cid) else {return None};

        let Some(variety_score) = self.variety_scores.get(cid) else {return None};
        let Some(length_score) = self.length_scores.get(cid) else {return None};
        let Some(lang_score) = self.lang_scores.get(cid) else {return None};
        let popularity_score = Score::from(providers.len() as f64 / max_provider_count);
        let ipns_score = Score::from(result.has_ipns() as usize as f64);
        let verified_score = Score::from(self.verified.contains(cid) as usize as f64);

        Some(Scores {
            tf_score,
            variety_score: *variety_score,
            length_score: *length_score,
            lang_score: *lang_score,
            popularity_score,
            ipns_score,
            verified_score,
        })
    }

    fn get_index_path(&self, cid: &String) -> Option<Vec<String>> {
        let mut path = self.results.get(cid).and_then(|r| r.paths.first())?.to_owned();
        if !path.last().map(|l| l=="index.html").unwrap_or(false) {
            return None;
        }
        path.pop();
        Some(path)
    }

    pub fn rerank(&mut self) {
        // Recompute TF scores
        let res_count = self.results.len() as f64;
        let mut tf_scores = HashMap::new();
        for (i, (cid, _)) in self.tf_ranking.iter().enumerate() {
            tf_scores.insert(cid, i as f64 / res_count);
        }

        // Group results
        log!("{} grouping results", self.grouping_results.len());
        let mut groups = HashMap::new();
        for parent_cid in self.grouping_results.iter() {
            let Some(path) = self.get_index_path(parent_cid) else {continue}; 
            groups.insert(path, (parent_cid, Vec::new())); // TODO: handle the case where 2 results claim to have the same path
        }

        // List ungrouped results
        let mut ungrouped = HashSet::new();
        'grouping: for (cid, result) in self.results.iter().filter(|(cid,_)| !self.grouping_results.contains(*cid)) {
            let Some(path) = result.paths.first() else {continue};
            let mut path = path.as_slice();
            loop {
                if path.is_empty() {
                    break;
                }
                path = &path[..path.len()-1];
                if let Some((_, cids)) = groups.get_mut(path) {
                    cids.push(cid);
                    continue 'grouping;
                }
            }
            ungrouped.insert(cid);
        }
        log!("{} ungrouped results", ungrouped.len());

        // Disband small groups
        'disbanding: for grouping_result in &self.grouping_results {
            let Some(path) = self.get_index_path(grouping_result) else {continue};

            let (parent, children) = groups.get(&path).unwrap();
            if children.len() <= 3 { // TODO: make this configurable
                let Some(mut path) = self.get_index_path(parent) else {continue}; 
                let (parent, children) = groups.remove(&path).unwrap();
                loop {
                    if path.is_empty() {
                        break;
                    }
                    path.pop();
                    if let Some((_, cids)) = groups.get_mut(&path) {
                        cids.push(parent);
                        cids.extend(children);
                        continue 'disbanding;
                    }
                }
                ungrouped.insert(parent);
                ungrouped.extend(children);
            }
        }

        // Compute scores and rank groups
        self.fully_ranked = Vec::new();
        for (parent_cid, cids) in groups.into_values().chain(ungrouped.into_iter().map(|cid| (cid, Vec::new()))) {
            let Some(parent_scores) = self.get_scores(parent_cid, Score::from(tf_scores[&parent_cid])) else {continue};
            let mut grouped_results = GroupedResults::new((parent_cid.to_owned(), parent_scores));
            for cid in cids {
                let Some(scores) = self.get_scores(cid, Score::from(tf_scores[&cid])) else {continue};
                grouped_results.insert(cid.to_owned(), scores);
            }
            let i = self.fully_ranked.binary_search_by_key(&grouped_results.scores(), |others| others.scores()).unwrap_or_else(|i| i);
            self.fully_ranked.insert(i, grouped_results);
        }
    }

    pub fn verify_some(&mut self, top: usize, search_id: u64, ctx: &Context<ResultsPage>) {
        /*let rpc_addr = ctx.props().conn_status.rpc_addr();
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
        }*/
    }

    pub fn malicious_result(&mut self, cid: String) {
        self.results.remove(&cid);
        self.grouping_results.remove(&cid);
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

    pub fn get_ranked(&self) -> &[GroupedResults] {
        self.fully_ranked.as_slice()
    }

    pub fn iter_with_scores(&self) -> impl Iterator<Item = Vec<(DocumentResult, Scores)>> + '_ {
        self.fully_ranked.iter().filter_map(|refs| refs.to_docs(&self.results))
    }
}
