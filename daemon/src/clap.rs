use crate::prelude::*;
use clap::ArgAction::Set;

/// Admarus search engine daemon
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Address on which the Admarus node will listen
    #[arg(long, default_values_t = [String::from("/ip4/0.0.0.0/tcp/4002"), String::from("/ip6/::/tcp/4002")])]
    pub listen_addrs: Vec<String>,

    /// External addrs to advertise.
    /// Domains will be resolved at startup (but specifying port is not supported yet).
    #[arg(long)]
    pub external_addrs: Option<Vec<String>>,

    /// IPFS RPC url
    #[arg(long, default_value = "http://localhost:5001")]
    pub ipfs_rpc: String,

    /// Enables getting peers from IPFS
    #[arg(long, default_value = "false", action = Set)]
    pub ipfs_peers_enabled: bool,

    /// Census public RPC url
    #[arg(long, default_value = "https://census.admarus.net")]
    pub census_rpc: String,

    /// Enables census RPC server
    #[arg(long, default_value = "true", action = Set)]
    pub census_enabled: bool,

    /// Address on which the API will listen
    #[arg(long, default_value_t = String::from("127.0.0.1:5002"))]
    pub api_addr: String,

    /// Domain names to keep pinned
    #[arg(long)]
    pub dns_pins: Vec<String>,

    /// Update interval for DNS pins (in seconds)
    #[arg(long, default_value = "1800")]
    pub dns_pins_interval: u64,

    /// Address of the DNS provider
    #[arg(long, default_value = "8.8.8.8:53")]
    pub dns_provider: String,

    /// Number of seeders to connect to
    #[arg(long, default_value = "8")]
    pub first_class: usize,

    /// Number of leechers to allow to connect
    #[arg(long, default_value = "50")]
    pub leechers: usize,
}

pub async fn resolve_external_addrs(addrs: &mut Vec<String>, dns_provider: SocketAddr) {
    let (stream, sender) = TcpClientStream::<AsyncIoTokioAsStd<TokioTcpStream>>::new(dns_provider);
    let client = AsyncClient::new(stream, sender, None);
    let Ok((mut client, bg)) = client.await else {
        error!("Failed to connect to DNS provider");
        return;
    };
    tokio::spawn(bg);

    let mut queries = Vec::new();
    for i in (0..addrs.len()).rev() {
        let addr = &addrs[i];
        if addr.parse::<Multiaddr>().is_err() {
            let addr = addrs.remove(i);
            let Ok(name) = Name::from_str(&addr) else {
                error!("Invalid external addr (should be formatted as multiaddr): {addr}");
                continue;
            };
            let a_query = client.query(name.clone(), DNSClass::IN, RecordType::A);
            let aaaa_query = client.query(name, DNSClass::IN, RecordType::AAAA);
            queries.push(a_query);
            queries.push(aaaa_query);
        }
    }
    let results = join_all(queries).await;

    for resp in results.into_iter().filter_map(|res| res.ok()) {
        for answer in resp.answers() {
            match answer.data() {
                Some(RData::A(ip)) => {
                    let addr = format!("/ip4/{ip}/tcp/4002");
                    if !addrs.contains(&addr) {
                        debug!("Resolved new external addr: {addr}");
                        addrs.push(addr);
                    }
                }
                Some(RData::AAAA(ip)) => {
                    let addr = format!("/ip6/{ip}/tcp/4002");
                    if !addrs.contains(&addr) {
                        debug!("Resolved new external addr: {addr}");
                        addrs.push(addr);
                    }
                }
                _ => {}
            }
        }
    }
}
