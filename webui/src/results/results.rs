use crate::prelude::*;

const RANDOM_QUERIES: &[&str] = &["ipfs", "rust language", "bitcoin", "blog", "founder", "libp2p", "filecoin", "protocol labs", "peer to peer", "github"];

#[derive(Properties, Clone)]
pub struct ResultsPageProps {
    pub app_link: AppLink,
    pub query: Rc<String>,
}

impl PartialEq for ResultsPageProps {
    fn eq(&self, other: &Self) -> bool {
        self.query == other.query
    }
}

pub struct ResultsPage {
    query: Vec<String>,
    search_id: Option<u64>,
    search_failure: Option<ApiError>,
    update_counter: u32,
    results: RankedResults,
}

pub enum ResultsMessage {
    SearchSuccess(u64),
    SearchFailure(ApiError),
    FetchResultsSuccess(Vec<(DocumentResult, String)>),
    FetchResultsFailure(ApiError),
}

impl Component for ResultsPage {
    type Message = ResultsMessage;
    type Properties = ResultsPageProps;

    fn create(ctx: &Context<Self>) -> Self {
        let query = Rc::clone(&ctx.props().query);
        let link = ctx.link().clone();
        spawn_local(async move {
            match search(query.as_ref()).await {
                Ok(id) => link.send_message(ResultsMessage::SearchSuccess(id)),
                Err(e) => link.send_message(ResultsMessage::SearchFailure(e)),
            }
        });

        Self {
            query: ctx.props().query.split_whitespace().map(|s| s.to_string()).collect(),
            search_id: None,
            search_failure: None,
            update_counter: 0,
            results: RankedResults::new(),
        }
    }
    
    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            ResultsMessage::SearchSuccess(search_id) => {
                let link = ctx.link().clone();
                self.search_id = Some(search_id);
                spawn_local(async move {
                    sleep(Duration::from_secs(1)).await;
                    match fetch_results(search_id).await {
                        Ok(results) => link.send_message(ResultsMessage::FetchResultsSuccess(results)),
                        Err(e) => link.send_message(ResultsMessage::FetchResultsFailure(e)),
                    }
                });
                false
            }
            ResultsMessage::FetchResultsSuccess(results) => {
                self.update_counter += 1;
                for (result, provider) in results {
                    self.results.insert(result, provider, &self.query);
                }
                if let Some(search_id) = self.search_id {
                    let link = ctx.link().clone();
                    spawn_local(async move {
                        sleep(Duration::from_secs(1)).await;
                        match fetch_results(search_id).await {
                            Ok(results) => link.send_message(ResultsMessage::FetchResultsSuccess(results)),
                            Err(e) => link.send_message(ResultsMessage::FetchResultsFailure(e)),
                        }
                    });
                }
                true
            }
            ResultsMessage::SearchFailure(e) | ResultsMessage::FetchResultsFailure(e) => {
                log!("search failure: {e:?}"); // TODO: display error
                false
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        *self = Component::create(ctx);
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let results = self.results.iter_with_scores().collect::<Vec<_>>();
        let random_query = RANDOM_QUERIES[self.search_id.unwrap_or(0) as usize % RANDOM_QUERIES.len()];

        template_html!(
            "results/results.html",
            query = { ctx.props().query.to_string() },
            onsearch = { ctx.props().app_link.callback(|query| AppMsg::ChangePage(Page::Results(Rc::new(query)))) },
            onclick_home = { ctx.props().app_link.callback(|_| AppMsg::ChangePage(Page::Home)) },
            onclick_settings = { ctx.props().app_link.callback(|_| AppMsg::ChangePage(Page::Settings)) },

            no_results = { results.is_empty() && self.update_counter >= 10 },
            many_keywords = { ctx.props().query.split_whitespace().count() >= 1 },
            random_query,
            onclick_search_random = { ctx.props().app_link.callback(move |_| AppMsg::ChangePage(Page::Results(Rc::new(String::from(random_query))))) },

            addr_iter = { results.iter().map(|(result,_)| result.format_best_addr()) },
            addr2_iter = { results.iter().map(|(result,_)| result.format_best_addr()) },
            title_iter = { results.iter().map(|(result,_)| format!("{}", result.title)) },
            description_iter = { results.iter().map(|(result,_)| format!("{}", result.description)) },
            
            term_frequency_score_iter = { results.iter().map(|(_, scores)| scores.tf_score) },
            length_score_iter = { results.iter().map(|(_, scores)| scores.length_score) },
            popularity_score_iter = { results.iter().map(|(_, scores)| scores.popularity_score) },
            display_scores = true,
        )
    }
}
