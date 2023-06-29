use crate::prelude::*;

#[derive(Properties, PartialEq)]
pub struct SearchBarProps {
    pub onsearch: Callback<String>,
    #[prop_or_default]
    pub value: Option<String>,
}

pub enum SearchBarMsg {
    Search,
    Input(yew::InputEvent),
}

pub struct SearchBar {
    _onkeypress: Closure<dyn FnMut(web_sys::KeyboardEvent)>,
    value: String,
}

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
            value: ctx.props().value.clone().unwrap_or_default(),
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            SearchBarMsg::Search => {
                ctx.props().onsearch.emit(self.value.clone());
                false
            },
            SearchBarMsg::Input(e) => {
                let target = e.target().unwrap();
                self.value = target.dyn_ref::<web_sys::HtmlInputElement>().unwrap().value();
                false
            },
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, old_props: &Self::Properties) -> bool {
        if ctx.props().value != old_props.value {
            self.value = ctx.props().value.clone().unwrap_or_default();
            true
        } else {
            false
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        template_html!(
            "components/search_bar/search_bar.html",
            onclick_search = { ctx.link().callback(|_| SearchBarMsg::Search) },
            oninput = { ctx.link().callback(SearchBarMsg::Input) },
            value = { self.value.clone() },
        )
    }
}

