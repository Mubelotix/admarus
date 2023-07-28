use crate::prelude::*;

#[derive(Debug, PartialEq, Default, Clone)]
pub enum ServiceHealth {
    #[default]
    Unknown,
    Loading,
    Ok,
    Err(String),
}

impl ServiceHealth {
    pub fn is_unknown(&self) -> bool {
        matches!(self, ServiceHealth::Unknown)
    }

    pub fn is_err(&self) -> bool {
        matches!(self, ServiceHealth::Err(_))
    }
}

#[derive(Debug, PartialEq, Properties, Default, Clone)]
pub struct ConnectionStatus {
    pub admarus_daemon: ServiceHealth,
    pub admarus_gateway: ServiceHealth,
    pub ipfs_daemon: ServiceHealth,
    pub ipfs_gateway: ServiceHealth,
}

impl ConnectionStatus {
    pub fn admarus_addr(&self) -> &'static str {
        match self {
            ConnectionStatus { admarus_daemon: ServiceHealth::Ok, .. } => "http://127.0.0.1:5002",
            ConnectionStatus { admarus_gateway: ServiceHealth::Ok, .. } => "https://gateway.admarus.net",
            _ => "http://127.0.0.1:5002",
        }
    }

    pub fn ipfs_addr(&self) -> &'static str {
        match self {
            ConnectionStatus { ipfs_daemon: ServiceHealth::Ok, .. } => "http://localhost:8080",
            ConnectionStatus { ipfs_gateway: ServiceHealth::Ok, .. } => "https://dweb.link",
            _ => "https://dweb.link"
        }
    }

    pub fn apply_change(&mut self, change: ConnectionStatusChange) {
        match change {
            ConnectionStatusChange::AdmarusDaemon(admarus_daemon) => self.admarus_daemon = admarus_daemon,
            ConnectionStatusChange::AdmarusGateway(admarus_gateway) => self.admarus_gateway = admarus_gateway,
            ConnectionStatusChange::IpfsDaemon(ipfs_daemon) => self.ipfs_daemon = ipfs_daemon,
            ConnectionStatusChange::IpfsGateway(ipfs_gateway) => self.ipfs_gateway = ipfs_gateway,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConnectionStatusChange {
    AdmarusDaemon(ServiceHealth),
    AdmarusGateway(ServiceHealth),
    IpfsDaemon(ServiceHealth),
    IpfsGateway(ServiceHealth),
}

#[derive(Debug, PartialEq, Properties)]
pub struct ConnectionStatusProps {
    pub conn_status: Rc<ConnectionStatus>,
    pub onchange: Callback<ConnectionStatusChange>,
}

pub struct ConnectionStatusComp {

}

impl Component for ConnectionStatusComp {
    type Message = ();
    type Properties = ConnectionStatusProps;

    fn create(ctx: &Context<Self>) -> Self {
        let conn_status = ctx.props().conn_status.deref();
        if conn_status.admarus_daemon.is_unknown() {
            let onchange = ctx.props().onchange.clone();
            onchange.emit(ConnectionStatusChange::AdmarusDaemon(ServiceHealth::Loading));
            spawn_local(async move {
                onchange.emit(ConnectionStatusChange::AdmarusDaemon(match get_api_version("http://127.0.0.1:5002").await {
                    Ok(0) => ServiceHealth::Ok,
                    Ok(_) => ServiceHealth::Err(String::from("Daemon runs an incompatible api version")),
                    Err(e) => ServiceHealth::Err(format!("Failed to connect to daemon: {e:?}"))
                }));
            })
        }
        if conn_status.admarus_daemon.is_err() && conn_status.admarus_gateway.is_unknown() {
            let onchange = ctx.props().onchange.clone();
            onchange.emit(ConnectionStatusChange::AdmarusGateway(ServiceHealth::Loading));
            spawn_local(async move {
                onchange.emit(ConnectionStatusChange::AdmarusGateway(match get_api_version("https://gateway.admarus.net").await {
                    Ok(0) => ServiceHealth::Ok,
                    Ok(_) => ServiceHealth::Err(String::from("Gateway runs an incompatible api version")),
                    Err(e) => ServiceHealth::Err(format!("Failed to connect to gateway: {e:?}")),
                }));
            })
        }
        if conn_status.ipfs_daemon.is_unknown() {
            let onchange = ctx.props().onchange.clone();
            onchange.emit(ConnectionStatusChange::IpfsDaemon(ServiceHealth::Loading));
            spawn_local(async move {
                onchange.emit(ConnectionStatusChange::IpfsDaemon(match check_ipfs("http://localhost:8080").await {
                    Ok(()) => ServiceHealth::Ok,
                    Err(e) => ServiceHealth::Err(format!("Failed to connect to daemon: {e:?}"))
                }));
            })
        }
        if conn_status.ipfs_daemon.is_err() && conn_status.ipfs_gateway.is_unknown() {
            let onchange = ctx.props().onchange.clone();
            onchange.emit(ConnectionStatusChange::IpfsGateway(ServiceHealth::Loading));
            spawn_local(async move {
                onchange.emit(ConnectionStatusChange::IpfsGateway(match check_ipfs("https://dweb.link").await {
                    Ok(()) => ServiceHealth::Ok,
                    Err(e) => ServiceHealth::Err(format!("Failed to connect to gateway: {e:?}"))
                }));
            })
        }

        Self {}
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        *self = Component::create(ctx);
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let connected = matches!(
            ctx.props().conn_status.deref(),
            ConnectionStatus { admarus_daemon: ServiceHealth::Ok, ipfs_daemon: ServiceHealth::Ok, .. } |
            ConnectionStatus { admarus_daemon: ServiceHealth::Ok, ipfs_gateway: ServiceHealth::Ok, .. } |
            ConnectionStatus { admarus_gateway: ServiceHealth::Ok, ipfs_daemon: ServiceHealth::Ok, .. } |
            ConnectionStatus { admarus_gateway: ServiceHealth::Ok, ipfs_gateway: ServiceHealth::Ok, .. }
        );

        let error = !connected && matches!(
            ctx.props().conn_status.deref(),
            ConnectionStatus { admarus_daemon: ServiceHealth::Err(_), admarus_gateway: ServiceHealth::Err(_), .. } |
            ConnectionStatus { ipfs_daemon: ServiceHealth::Err(_), ipfs_gateway: ServiceHealth::Err(_), .. }
        );

        let connecting = !connected && !error && matches!(
            ctx.props().conn_status.deref(),
            ConnectionStatus { admarus_daemon: ServiceHealth::Loading, .. } |
            ConnectionStatus { admarus_gateway: ServiceHealth::Loading, .. } |
            ConnectionStatus { ipfs_daemon: ServiceHealth::Loading, .. } |
            ConnectionStatus { ipfs_gateway: ServiceHealth::Loading, .. }
        );

        let admarus_state = match ctx.props().conn_status.deref() {
            ConnectionStatus { admarus_daemon: ServiceHealth::Ok | ServiceHealth::Loading, .. } => "daemon",
            ConnectionStatus { admarus_gateway: ServiceHealth::Ok | ServiceHealth::Loading, .. } => "gateway",
            ConnectionStatus { admarus_daemon: ServiceHealth::Err(_), admarus_gateway: ServiceHealth::Err(_), .. } => "disconnected",
            _ => "unknown",
        };
        let ipfs_state = match ctx.props().conn_status.deref() {
            ConnectionStatus { ipfs_daemon: ServiceHealth::Ok | ServiceHealth::Loading, .. } => "daemon",
            ConnectionStatus { ipfs_gateway: ServiceHealth::Ok | ServiceHealth::Loading, .. } => "gateway",
            ConnectionStatus { ipfs_daemon: ServiceHealth::Err(_), ipfs_gateway: ServiceHealth::Err(_), .. } => "disconnected",
            _ => "unknown",
        };
        let state = if admarus_state == ipfs_state {
            format!("{admarus_state}s")
        } else {
            format!("Admarus {admarus_state} + IPFS {ipfs_state}")
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
