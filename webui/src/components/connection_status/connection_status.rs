use std::ops::Deref;
use crate::prelude::*;

#[derive(Debug, PartialEq, Properties, Default, Clone)]
pub struct ConnectionStatus {
    pub daemon: Option<Result<(), String>>,
    pub gateway: Option<Result<(), String>>,
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
                    match get_api_version().await {
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
                    match get_api_version().await {
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

    fn view(&self, ctx: &Context<Self>) -> Html {
        let (target, connected, connecting, error) = match ctx.props().conn_status.deref() {
            ConnectionStatus { daemon: Some(Err(_)), gateway: Some(Err(_)) } => ("gateway", false, false, true),
            ConnectionStatus { daemon: Some(Err(_)), gateway: Some(Ok(_)) } => ("gateway", false, true, false),
            ConnectionStatus { daemon: Some(Err(_)), gateway: None } => ("gateway", true, false, false),
            ConnectionStatus { daemon: Some(Ok(_)), .. } => ("daemon", true, false, false),
            ConnectionStatus { daemon: None, .. } => ("daemon", false, true, false),
        };

        template_html!("components/connection_status/connection_status.html", ...)
    }
}
