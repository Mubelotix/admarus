use crate::prelude::*;

/// Admarus search engine daemon
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Address on which the Kamilata node will listen
    #[arg(long, default_value_t = String::from("/ip4/127.0.0.1/tcp/4002"))]
    pub kam_addr: String,

    /// Address of the Kamilata bootstrap node
    #[arg(long)]
    pub kam_bootstrap: Option<String>,

    /// IPFS RPC url
    #[arg(long, default_value = "http://localhost:5001")]
    pub ipfs_rpc: String,

    /// Address on which the API will listen
    #[arg(long, default_value_t = String::from("127.0.0.1:3030"))]
    pub api_addr: String,
}
