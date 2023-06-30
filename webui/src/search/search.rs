use crate::prelude::*;

pub struct SearchPage {
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
    type Message = ();
    type Properties = SearchPageProps;

    fn create(_ctx: &Context<Self>) -> Self {
        Self {}
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let onsearch = ctx.props().app_link.callback(|query| AppMsg::ChangePage(Page::Results(Rc::new(query))));
        let onclick_settings = ctx.props().app_link.callback(|_| AppMsg::ChangePage(Page::Settings));
        let onclick_lucky = ctx.props().app_link.callback(|_| AppMsg::ChangePage(Page::lucky(None)));

        template_html!(
            "search/search.html",
            ...
        )
    }
}
