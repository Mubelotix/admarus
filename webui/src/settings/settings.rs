use crate::prelude::*;

pub struct SettingsPage {
    
}

#[derive(Properties, Clone)]
pub struct SettingsProps {
    pub app_link: AppLink
}

impl PartialEq for SettingsProps {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Component for SettingsPage {
    type Message = ();
    type Properties = SettingsProps;

    fn create(ctx: &Context<Self>) -> Self {
        Self {
            
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        template_html!(
            "src/settings/settings.html",
            onclick_search = { ctx.props().app_link.animate_callback(|_| AppMsg::ChangePage(Page::Search)) }
        )
    }
}
