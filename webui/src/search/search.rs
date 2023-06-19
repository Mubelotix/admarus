use crate::prelude::*;

pub struct SearchPage {
    
}

pub enum SearchPageMessage {
    LaunchSearch
}

#[derive(Properties, Clone)]
pub struct SearchPageProps {
    pub app_link: AppLink
}

impl PartialEq for SearchPageProps {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Component for SearchPage {
    type Message = SearchPageMessage;
    type Properties = SearchPageProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            SearchPageMessage::LaunchSearch => {
                let document = window().document().unwrap();
                let el = document.get_element_by_id("search-query-input").unwrap();
                let el: HtmlInputElement = el.dyn_into().unwrap();
                let query = Rc::new(el.value());
                ctx.props().app_link.animate_message(AppMsg::ChangePage(Page::Results(query)));
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        template_html!(
            "src/search/search.html",
            onclick_glass = { ctx.link().callback(|_| SearchPageMessage::LaunchSearch) },
            onclick_settings = { ctx.props().app_link.animate_callback(|_| AppMsg::ChangePage(Page::Settings)) }
        )
    }
}
