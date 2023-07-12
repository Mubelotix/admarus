use crate::prelude::*;

pub struct SearchPage {
}

#[derive(Properties, Clone)]
pub struct SearchPageProps {
    pub app_link: AppLink,
    pub conn_status: Rc<ConnectionStatus>,
    pub onchange_conn_status: Callback<ConnectionStatus>,
}

impl PartialEq for SearchPageProps {
    fn eq(&self, other: &Self) -> bool {
        self.conn_status == other.conn_status
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
        let conn_status = Rc::clone(&ctx.props().conn_status);
        let onchange_conn_status = ctx.props().onchange_conn_status.clone();

        template_html!(
            "search/search.html",
            ...
        )
    }
}
