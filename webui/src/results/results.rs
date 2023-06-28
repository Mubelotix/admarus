use crate::prelude::*;

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
    search_id: Option<u64>,
    search_failure: Option<ApiError>,
    results: Vec<DocumentResult>
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
            search_id: None,
            search_failure: None,
            results: Vec::new(),
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
                let new_results = !results.is_empty();
                for (result, _) in results {
                    let i = self.results.binary_search_by_key(&result.score(), |r| r.score()).unwrap_or_else(|i| i);
                    self.results.insert(i, result);
                }
                if new_results {
                    for result in &self.results {
                        log!("result: {} {} {}", result.title, result.score(), result.word_count.sum());
                    }
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

    fn view(&self, ctx: &Context<Self>) -> Html {
        template_html!(
            "results/results.html",
            query = { ctx.props().query.to_string() },
            onsearch = { ctx.props().app_link.callback(|query| AppMsg::ChangePage(Page::Results(Rc::new(query)))) },
            onclick_home = { ctx.props().app_link.callback(|_| AppMsg::ChangePage(Page::Home)) },
            onclick_settings = { ctx.props().app_link.callback(|_| AppMsg::ChangePage(Page::Settings)) },
            addr_iter = { self.results.iter().rev().map(|result| format!("ipfs://{}", result.paths.first().map(|p| p.join("/")).unwrap_or(result.cid.clone()))) },
            addr2_iter = { self.results.iter().rev().map(|result| format!("ipfs://{}", result.paths.first().map(|p| p.join("/")).unwrap_or(result.cid.clone()))) },
            title_iter = { self.results.iter().rev().map(|result| format!("{}", result.title)) },
            description_iter = { self.results.iter().rev().map(|result| format!("{}", result.description)) },
        )
    }
}
