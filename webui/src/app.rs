use std::rc::Rc;

use yew::virtual_dom::{VChild, VNode, VComp};

use crate::prelude::*;

#[derive(Clone)]
pub enum Page {
    Home,
    Results(Rc<String>),
    Settings,
}

#[derive(Clone)]
pub enum AppMsg {
    ChangePage(Page),
}

pub struct App {
    page: Page,
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            page: Page::Home,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMsg::ChangePage(page) => {
                self.page = page;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        match self.page {
            Page::Home => html!(<SearchPage app_link={ctx.link().clone()} />),
            Page::Settings => html!(<SettingsPage app_link={ctx.link().clone()} />),
            Page::Results(ref query) => html!(<ResultsPage app_link={ctx.link().clone()} query={Rc::clone(query)} />),
        }
    }
}
