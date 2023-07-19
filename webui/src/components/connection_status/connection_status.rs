use crate::prelude::*;

#[derive(Debug, PartialEq, Properties, Default, Clone)]
pub struct ConnectionStatus {
    pub daemon: Option<Result<(), String>>,
    pub gateway: Option<Result<(), String>>,
}

impl ConnectionStatus {
    pub fn rpc_addr(&self) -> &'static str {
        match self {
            ConnectionStatus { daemon: Some(Ok(_)), .. } => "http://127.0.0.1:5002",
            ConnectionStatus { gateway: Some(Ok(_)), .. } => "https://gateway.admarus.net",
            _ => "http://127.0.0.1:5002",
        }
    }
}

#[derive(Debug, PartialEq, Properties)]
pub struct ConnectionStatusProps {
    pub conn_status: Rc<ConnectionStatus>,
    pub onchange: Callback<ConnectionStatus>,
}

pub struct ConnectionStatusComp {

}

impl Component for ConnectionStatusComp {
    type Message = ();
    type Properties = ConnectionStatusProps;

    fn create(ctx: &Context<Self>) -> Self {
        match ctx.props().conn_status.deref() {
            ConnectionStatus { daemon: None, .. } => {
                let onchange = ctx.props().onchange.clone();
                spawn_local(async move {
                    match get_api_version("http://127.0.0.1:5002").await {
                        Ok(0) => {
                            onchange.emit(ConnectionStatus {
                                daemon: Some(Ok(())),
                                gateway: None,
                            });
                        },
                        Ok(_) => {
                            onchange.emit(ConnectionStatus {
                                daemon: Some(Err(String::from("Daemon runs an incompatible api version"))),
                                gateway: None,
                            });
                        },
                        Err(e) => {
                            onchange.emit(ConnectionStatus {
                                daemon: Some(Err(format!("Failed to connect to daemon: {e:?}"))),
                                gateway: None,
                            });
                        }
                    }
                })
            },
            ConnectionStatus { daemon: Some(Err(daemon_error)), gateway: None } => {
                let daemon = Some(Err(daemon_error.clone()));
                let onchange = ctx.props().onchange.clone();
                spawn_local(async move {
                    match get_api_version("https://gateway.admarus.net").await {
                        Ok(0) => {
                            onchange.emit(ConnectionStatus {
                                daemon,
                                gateway: Some(Ok(())),
                            });
                        },
                        Ok(_) => {
                            onchange.emit(ConnectionStatus {
                                daemon,
                                gateway: Some(Err(String::from("Gateway runs an incompatible api version"))),
                            });
                        },
                        Err(e) => {
                            onchange.emit(ConnectionStatus {
                                daemon,
                                gateway: Some(Err(format!("Failed to connect to gateway: {e:?}"))),
                            });
                        }
                    }
                })
            },
            _ => ()
        }

        Self {}
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        *self = Component::create(ctx);
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let (target, connected, connecting, error) = match ctx.props().conn_status.deref() {
            ConnectionStatus { daemon: Some(Err(_)), gateway: Some(Err(_)) } => ("disconnected", false, false, true),
            ConnectionStatus { daemon: Some(Err(_)), gateway: Some(Ok(_)) } => ("gateway", true, false, false),
            ConnectionStatus { daemon: Some(Err(_)), gateway: None } => ("gateway", false, true, false),
            ConnectionStatus { daemon: Some(Ok(_)), .. } => ("daemon", true, false, false),
            ConnectionStatus { daemon: None, .. } => ("daemon", false, true, false),
        };

        // Storing the SVG here is necessary to prevent yew from lowering the case of SVG attributes
        let svg_success = VNode::from_html_unchecked(r#"
            <svg class="checkmark" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 52 52">
                <circle class="checkmark__circle" cx="26" cy="26" r="25" fill="none"/>
                <path class="checkmark__check" fill="none" d="M14.1 27.2l7.1 7.2 16.7-16.8"/>
            </svg>"#
            .into()
        );
        let svg_error = VNode::from_html_unchecked(r#"
            <svg class="checkmark" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 52 52" fill="none" stroke-linecap="round" stroke-linejoin="round">
                <circle class="checkmark__circle" cx="26" cy="26" r="25"/>
                <line class="checkmark__check" x1="15" y1="15" x2="37" y2="37"/>
                <line class="checkmark__check" x1="15" y1="37" x2="37" y2="15"/>
            </svg>"#
            .into()
        );

        template_html!("components/connection_status/connection_status.html", ...)
    }
}
