use crate::prelude::*;

/// Admarus search engine daemon
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Address on which the Admarus node will listen
    #[arg(long, default_values_t = [String::from("/ip4/0.0.0.0/tcp/4002"), String::from("/ip6/::/tcp/4002")])]
    pub listen_addrs: Vec<String>,

    /// External addrs to advertise
    #[arg(long)]
    pub external_addrs: Option<Vec<String>>,

    /// IPFS RPC url
    #[arg(long, default_value = "http://localhost:5001")]
    pub ipfs_rpc: String,

    /// Census public RPC url
    #[arg(long, default_value = "http://localhost:14364")]
    pub census_rpc: Option<String>,

    /// Address on which the API will listen
    #[arg(long, default_value_t = String::from("127.0.0.1:3030"))]
    pub api_addr: String,

    /// Number of seeders to connect to
    #[arg(long, default_value = "8")]
    pub first_class: usize,

    /// Number of leechers to allow to connect
    #[arg(long, default_value = "50")]
    pub leechers: usize,
}