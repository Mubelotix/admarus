use std::collections::HashSet;

use crate::prelude::*;

#[derive(Properties, Clone)]
pub struct ResultsPageProps {
    pub app_link: AppLink,
    pub query: Rc<String>,
    pub conn_status: Rc<ConnectionStatus>,
    pub onchange_conn_status: Callback<ConnectionStatus>,
}

impl PartialEq for ResultsPageProps {
    fn eq(&self, other: &Self) -> bool {
        self.query == other.query && self.conn_status == other.conn_status
    }
}

pub enum DocumentType {
    All,
    Documents,
    Images,
    Videos,
}

pub struct ResultsPage {
    document_type: DocumentType,
    search_data: Option<(u64, Query)>,
    search_error: Option<ApiError>,
    update_counter: u32,
    results: RankedResults,
    providers: HashSet<String>,
}

pub enum ResultsMessage {
    SelectDocumentType(DocumentType),
    SearchSuccess(ApiSearchResponse),
    SearchFailure(ApiError),
    FetchResultsSuccess { search_id: u64, results: Vec<(DocumentResult, String)> },
    FetchResultsFailure(ApiError),
    MaliciousResult(String),
    VerifiedResult(String, Box<DocumentResult>),
}

impl Component for ResultsPage {
    type Message = ResultsMessage;
    type Properties = ResultsPageProps;

    fn create(ctx: &Context<Self>) -> Self {
        let query = Rc::clone(&ctx.props().query);
        let link = ctx.link().clone();
        let rpc_addr = ctx.props().conn_status.rpc_addr();
        spawn_local(async move {
            match search(rpc_addr, query.as_ref()).await {
                Ok(id) => link.send_message(ResultsMessage::SearchSuccess(id)),
                Err(e) => link.send_message(ResultsMessage::SearchFailure(e)),
            }
        });

        Self {
            document_type: DocumentType::All,
            search_data: None,
            search_error: None,
            update_counter: 0,
            results: RankedResults::new(),
            providers: HashSet::new(),
        }
    }
    
    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            ResultsMessage::SelectDocumentType(document_type) => {
                self.document_type = document_type;
                true
            }
            ResultsMessage::SearchSuccess(resp) => {
                let link = ctx.link().clone();
                self.search_data = Some((resp.id, resp.query));
                let rpc_addr = ctx.props().conn_status.rpc_addr();
                spawn_local(async move {
                    sleep(Duration::from_millis(100)).await;
                    match fetch_results(rpc_addr, resp.id).await {
                        Ok(results) => link.send_message(ResultsMessage::FetchResultsSuccess { search_id: resp.id, results }),
                        Err(e) => link.send_message(ResultsMessage::FetchResultsFailure(e)),
                    }
                });
                false
            }
            ResultsMessage::FetchResultsSuccess { search_id: results_search_id, results } => {
                let Some((search_id, query)) = &self.search_data else { return false };
                let search_id = *search_id;
                let update_counter = self.update_counter;
                if results_search_id != search_id { return false };

                // Insert results and rank them
                self.update_counter += 1;
                let new_results = !results.is_empty();
                for (result, provider) in results {
                    self.results.insert(result, provider.clone(), query);
                    self.providers.insert(provider);
                }
                self.results.rerank();
                if new_results {
                    self.results.rerank();
                }

                let link = ctx.link().clone();
                let rpc_addr = ctx.props().conn_status.rpc_addr();
                spawn_local(async move {
                    match update_counter {
                        0..=10 => sleep(Duration::from_millis(100)).await,
                        11..=20 => sleep(Duration::from_millis(300)).await,
                        _ => sleep(Duration::from_secs(1)).await,
                    }
                    match fetch_results(rpc_addr, search_id).await {
                        Ok(results) => link.send_message(ResultsMessage::FetchResultsSuccess { search_id, results }),
                        Err(e) => link.send_message(ResultsMessage::FetchResultsFailure(e)),
                    }
                });

                true
            }
            ResultsMessage::MaliciousResult(cid) => {
                self.results.malicious_result(cid);
                true
            }
            ResultsMessage::VerifiedResult(cid, trusted_result) => {
                self.results.verified_result(cid, *trusted_result);
                true
            }
            ResultsMessage::SearchFailure(e) | ResultsMessage::FetchResultsFailure(e) => {
                self.search_error = Some(e);
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        *self = Component::create(ctx);
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let results = self.results.iter_with_scores().collect::<Vec<_>>();
        let search_id = self.search_data.as_ref().map(|d| d.0);
        let query = self.search_data.as_ref().map(|d| &d.1);

        // General
        let query_string = ctx.props().query.to_string();
        let onsearch = ctx.props().app_link.callback(move |query| AppMsg::ChangePage(Page::Results(Rc::new(query))));
        let onclick_home = ctx.props().app_link.callback(|_| AppMsg::ChangePage(Page::Home));
        let onclick_settings = ctx.props().app_link.callback(|_| AppMsg::ChangePage(Page::Settings));

        // Document type selectors
        let all_selected = matches!(self.document_type, DocumentType::All);
        let documents_selected = matches!(self.document_type, DocumentType::Documents);
        let all_or_documents_selected = all_selected || documents_selected;
        let images_selected = matches!(self.document_type, DocumentType::Images);
        let videos_selected = matches!(self.document_type, DocumentType::Videos);
        let all_selector_class = if all_selected { "selected" } else { "" };
        let documents_selector_class = if documents_selected { "selected" } else { "" };
        let images_selector_class = if images_selected { "selected" } else { "" };
        let videos_selector_class = if videos_selected { "selected" } else { "" };
        let onclick_select_all = ctx.link().callback(|_| ResultsMessage::SelectDocumentType(DocumentType::All));
        let onclick_select_documents = ctx.link().callback(|_| ResultsMessage::SelectDocumentType(DocumentType::Documents));
        let onclick_select_images = ctx.link().callback(|_| ResultsMessage::SelectDocumentType(DocumentType::Images));
        let onclick_select_videos = ctx.link().callback(|_| ResultsMessage::SelectDocumentType(DocumentType::Videos));

        // Result counter
        let opt_result_counter = match (results.len(), self.providers.len()) {
            (0, _) => None,
            (1, _) => Some(String::from("1 result")),
            (n, 1) => Some(format!("{} results from 1 provider", n)),
            (n, p) => Some(format!("{} results from {} providers", n, p)),
        };

        // No-results message
        let no_results = results.is_empty() && self.search_error.is_none() && self.update_counter >= 10;
        let many_keywords = ctx.props().query.split_whitespace().count() >= 3;
        let lucky_query = get_lucky_query(search_id);
        let onclick_lucky = ctx.props().app_link.callback(move |_| AppMsg::ChangePage(Page::lucky(search_id)));

        // Error message
        let (opt_error_title, error_recommandations, opt_error_details) = match &self.search_error {
            Some(e) => {
                let (title, recommandations, details) = e.to_format_parts();
                (Some(title), recommandations, Some(details))
            },
            None => (None, Vec::new(), None)
        };
        let error_recommandation_iter = error_recommandations.into_iter();

        // Connection status
        let conn_status = Rc::clone(&ctx.props().conn_status);
        let onchange_conn_status = ctx.props().onchange_conn_status.clone();

        let result_components: Html = match query {
            Some(query) => {
                let query = Rc::new(query.to_owned());
                results.into_iter().map(|results| {
                    html! {
                        <GroupedResultsComp results={results} query={Rc::clone(&query)} />
                    }
                }).collect()
            },
            None => html!(),
        };

        template_html!(
            "pages/results/results.html",
            onclick_home = { onclick_home.clone() },
            ...
        )
    }
}
