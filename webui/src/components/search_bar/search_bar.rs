use crate::prelude::*;

#[derive(Properties, PartialEq)]
pub struct SearchBarProps {
    pub onsearch: Callback<String>,
}

pub enum SearchBarMsg {
    Search,
}

pub struct SearchBar {
    _onkeypress: Closure<dyn FnMut(web_sys::KeyboardEvent)>,
}
pub use SearchBar as searchbar;

impl Component for SearchBar {
    type Message = SearchBarMsg;
    type Properties = SearchBarProps;

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link().clone();
        let onkeypress = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
            if e.key() == "Enter" {
                link.send_message(SearchBarMsg::Search);
            }
        }) as Box<dyn FnMut(_)>);
        wndw().add_event_listener_with_callback("keypress", onkeypress.as_ref().unchecked_ref()).unwrap();

        SearchBar {
            _onkeypress: onkeypress,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            SearchBarMsg::Search => {
                let document = wndw().document().unwrap();
                let el = document.get_element_by_id("search-query-input").unwrap();
                let el: HtmlInputElement = el.dyn_into().unwrap();
                let query = el.value();
                ctx.props().onsearch.emit(query);
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

