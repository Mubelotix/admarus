use crate::prelude::*;
use clap::ArgAction::Set;

/// Admarus search engine daemon
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Address on which the Admarus node will listen
    #[arg(long, default_values_t = [String::from("/ip4/0.0.0.0/tcp/4002"), String::from("/ip6/::/tcp/4002")])]
    pub listen_addrs: Vec<String>,

    /// External addrs to advertise encoded as Multiaddr
    /// Example with DNS: /dns4/domain.tld/tcp/4002
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

    /// Number of seeders to connect to
    #[arg(long, default_value = "8")]
    pub first_class: usize,

    /// Number of leechers to allow to connect
    #[arg(long, default_value = "50")]
    pub leechers: usize,

    /// Path to the database.
    /// Admarus does not require using a database, which is fine under 10000 documents.
    #[arg(long)]
    pub database_path: Option<String>
}
