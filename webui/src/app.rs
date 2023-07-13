use std::rc::Rc;

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
    ConnectionStatusChanged(ConnectionStatus),
}

pub struct App {
    page: Page,
    conn_status: Rc<ConnectionStatus>,
}

impl Component for App {
    type Message = AppMsg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            page: Page::Home,
            conn_status: Rc::new(ConnectionStatus::default()),
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AppMsg::ChangePage(page) => {
                self.page = page;
                true
            }
            AppMsg::ConnectionStatusChanged(conn_status) => {
                self.conn_status = Rc::new(conn_status);
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        match self.page {
            Page::Home => html!(
                <SearchPage
                    app_link={ctx.link().clone()}
                    conn_status={Rc::clone(&self.conn_status)}
                    onchange_conn_status={ctx.link().callback(AppMsg::ConnectionStatusChanged)} />
            ),
            Page::Settings => html!(<SettingsPage app_link={ctx.link().clone()} />),
            Page::Results(ref query) => html!(<ResultsPage app_link={ctx.link().clone()} query={Rc::clone(query)} />),
        }
    }
}
