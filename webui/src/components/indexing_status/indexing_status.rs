use std::cell::RefCell;

use crate::prelude::*;

mod ty;
pub use ty::*;
mod api;
pub use api::*;

pub struct IndexingStatusComp {
    status: Option<IndexingStatus>,
    stop_polling: Rc<RefCell<bool>>,
}

#[derive(PartialEq, Properties)]
pub struct IndexingStatusProps {
    pub rpc_addr: String,
}

pub enum IndexingStatusMsg {
    SetStatus(IndexingStatus),
}

impl Component for IndexingStatusComp {
    type Message = IndexingStatusMsg;
    type Properties = IndexingStatusProps;

    fn create(ctx: &Context<Self>) -> Self {
        let stop_polling = Rc::new(RefCell::new(false));

        let link2 = ctx.link().clone();
        let rpc_addr2 = ctx.props().rpc_addr.clone();
        let stop_polling2 = Rc::clone(&stop_polling);
        spawn_local(async move {
            loop {
                if *stop_polling2.borrow() {
                    break;
                }

                let status = match get_indexing_status(&rpc_addr2).await {
                    Ok(status) => status,
                    Err(e) => {
                        log!("Failed to get indexing status: {}", e);
                        sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };

                let idle = status.to_list == 0 && status.to_load == 0 && status.to_load_unprioritized == 0;
                let cooldown = if idle { 30 } else { 1 };

                link2.send_message(IndexingStatusMsg::SetStatus(status));
                sleep(Duration::from_secs(cooldown)).await;
            }
        });

        Self {
            status: None,
            stop_polling,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: IndexingStatusMsg) -> bool {
        match msg {
            IndexingStatusMsg::SetStatus(status) => {
                self.status = Some(status);
                true
            }
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        self.stop_polling.replace(true);
        *self = Component::create(ctx);
        true
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let (exploring, indexing, building_filter, progress_value, progress_max) = match &self.status {
            Some(status) if status.to_list > 0 => (
                true, false, false,
                Some(status.listed),
                status.listed + status.to_list,
            ),
            Some(status) if status.to_load + status.to_load_unprioritized > 0 => (
                false, true, false,
                Some(status.loaded),
                status.loaded + status.to_load + status.to_load_unprioritized,
            ),
            Some(status) if status.updating_filter => (
                false, false, true,
                None,
                1,
            ),
            _ => return html! {},
        };

        template_html!(
            "components/indexing_status/indexing_status.html",
            progress_max = {progress_max.to_string()},
            progress_value = {progress_value.unwrap_or(0).to_string()},
            ...
        )
    }
}
