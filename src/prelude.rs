pub use crate::{
    result::*,
    index::*,
    rpc_ipfs::*,
    rpc_census::*,
    documents::*,
    api::*,
    node::*,
    clap::*,
    swarm::*,
    discovery::{Behaviour as DiscoveryBehavior, Event as DiscoveryEvent, Config as DiscoveryConfig, Response as DiscoveryResponse, PeerListQuery}
};
pub use clap::Parser;
pub use log::{info, warn, error, debug, trace};
pub use kamilata::{prelude::*, db::TooManyLeechers};
pub use serde::{Serialize, Deserialize};
pub use async_trait::async_trait;
pub use std::{collections::HashMap, sync::Arc, time::{SystemTime, Duration, Instant, UNIX_EPOCH}, pin::Pin, future::Future, cmp::Ordering, io::Error as IoError};
pub use tokio::{sync::RwLock, time::sleep};
pub use libp2p::{PeerId, Multiaddr, swarm::dial_opts::DialOpts, multiaddr::Protocol, core::identity::Keypair};
pub use futures::future::{BoxFuture, join_all};
pub use reqwest::Client;
pub use sha2_derive::Hashable;

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}
