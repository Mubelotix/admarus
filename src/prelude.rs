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
pub use kamilata::{prelude::*, db::TooManyLeechers, behaviour::KamilataEvent};
pub use serde::{Serialize, Deserialize};
pub use async_trait::async_trait;
pub use std::{
    time::{SystemTime, Duration, Instant, UNIX_EPOCH},
    collections::HashMap, sync::Arc, pin::Pin, future::Future, cmp::Ordering, io::Error as IoError
};
pub use tokio::{
    sync::{RwLock, mpsc::*, oneshot::{Sender as OneshotSender, channel as oneshot_channel}},
    time::sleep
};
pub use libp2p::{
    swarm::{dial_opts::DialOpts, Swarm, SwarmBuilder, SwarmEvent, NetworkBehaviour, AddressRecord},
    core::{identity::Keypair, upgrade},
    PeerId, Multiaddr, multiaddr::Protocol, tcp, Transport, mplex::MplexConfig, noise
};
pub use futures::{
    future::{BoxFuture, join_all, Either},
    StreamExt
};
pub use reqwest::Client;
pub use sha2_derive::Hashable;
pub use libp2p_identify::{Behaviour as IdentifyBehaviour, Event as IdentifyEvent, Config as IdentifyConfig};

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}
