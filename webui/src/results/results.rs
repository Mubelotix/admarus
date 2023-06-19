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

}

pub enum ResultsMessage {
    RelaunchSearch,
}

impl Component for ResultsPage {
    type Message = ResultsMessage;
    type Properties = ResultsPageProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self {

        }
    }
    
    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            ResultsMessage::RelaunchSearch => {
                let document = window().document().unwrap();
                let el = document.get_element_by_id("search-query-input").unwrap();
                let el: HtmlInputElement = el.dyn_into().unwrap();
                let query = Rc::new(el.value());
                ctx.props().app_link.animate_message(AppMsg::ChangePage(Page::Results(query)));
                false
            },
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        template_html!(
            "results/results.html",
            query = { ctx.props().query.to_string() },
            onclick_glass = { ctx.link().callback(|_| ResultsMessage::RelaunchSearch) },
            onclick_settings = { ctx.props().app_link.animate_callback(|_| AppMsg::ChangePage(Page::Settings)) }
        )
    }
}
