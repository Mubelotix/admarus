use crate::prelude::*;

mod ty;
pub use ty::*;
mod api;
pub use api::*;

pub struct IndexingStatusComp {
    status: Option<IndexingStatus>,
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
        let link2 = ctx.link().clone();
        let rpc_addr2 = ctx.props().rpc_addr.clone();
        spawn_local(async move {
            loop {
                if link2.get_component().is_none() {
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
