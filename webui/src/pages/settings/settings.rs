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

    fn create(_ctx: &Context<Self>) -> Self {
        Self {
            
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        template_html!(
            "pages/settings/settings.html",
            onclick_search = { ctx.props().app_link.callback(|_| AppMsg::ChangePage(Page::Home)) }
        )
    }
}
