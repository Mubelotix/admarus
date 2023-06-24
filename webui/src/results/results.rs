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
    results: Vec<(DocumentResult, String)>
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
                self.results.extend(results);
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
            onsearch = { ctx.props().app_link.animate_callback(|query| AppMsg::ChangePage(Page::Results(Rc::new(query)))) },
            onclick_home = { ctx.props().app_link.animate_callback(|_| AppMsg::ChangePage(Page::Home)) },
            onclick_settings = { ctx.props().app_link.animate_callback(|_| AppMsg::ChangePage(Page::Settings)) },
            addr_iter = { self.results.iter().map(|result| format!("ipfs://{}", result.0.paths.first().map(|p| p.join("/")).unwrap_or(result.0.cid.clone()))) },
            addr2_iter = { self.results.iter().map(|result| format!("ipfs://{}", result.0.paths.first().map(|p| p.join("/")).unwrap_or(result.0.cid.clone()))) },
            title_iter = { self.results.iter().map(|result| format!("{}", result.0.title)) },
            description_iter = { self.results.iter().map(|result| format!("{}", result.0.description)) },
        )
    }
}
