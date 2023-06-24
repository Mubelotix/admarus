use crate::prelude::*;

#[derive(Properties, PartialEq)]
pub struct SearchBarProps {
    on_search: Callback<String>,
}

pub enum SearchBarMsg {
    Search,
}

pub struct SearchBar {

}

impl Component for SearchBar {
    type Message = SearchBarMsg;
    type Properties = SearchBarProps;

    fn create(ctx: &Context<Self>) -> Self {
        // TODO event listeners
        SearchBar {  }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            SearchBarMsg::Search => {
                let document = wndw().document().unwrap();
                let el = document.get_element_by_id("search-query-input").unwrap();
                let el: HtmlInputElement = el.dyn_into().unwrap();
                let query = el.value();
                ctx.props().on_search.emit(query);
                false
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        template_html!(
            "components/search_bar/search_bar.html",
            onclick_search = { ctx.link().callback(|_| SearchBarMsg::Search) },
        )
    }
}

