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
    dns_pins::*,
    query::*,
    discovery::{Behaviour as DiscoveryBehavior, Event as DiscoveryEvent, Config as DiscoveryConfig, Response as DiscoveryResponse, PeerListQuery}
};
pub use trust_dns_client::{
    client::{AsyncClient, ClientHandle},
    proto::iocompat::AsyncIoTokioAsStd,
    rr::{DNSClass, Name, RData, RecordType},
    tcp::TcpClientStream
};
pub use clap::Parser;
pub use log::{info, warn, error, debug, trace};
pub use kamilata::{prelude::*, db::TooManyLeechers, behaviour::KamilataEvent, store::{ResultStream, ResultStreamBuilderFut}};
pub use serde::{Serialize, Deserialize};
pub use async_trait::async_trait;
pub use std::{
    time::{SystemTime, Duration, Instant, UNIX_EPOCH},
    collections::{HashMap, HashSet}, sync::Arc, pin::Pin, future::Future, cmp::Ordering, iter::zip, net::SocketAddr, str::FromStr, io::Error as IoError,
};
pub use tokio::{
    sync::{RwLock, mpsc::*, oneshot::{Sender as OneshotSender, channel as oneshot_channel}},
    time::{sleep, timeout},
    net::TcpStream as TokioTcpStream
};
pub use libp2p::{
    swarm::{dial_opts::DialOpts, Swarm, SwarmBuilder, SwarmEvent, NetworkBehaviour, AddressRecord},
    core::{identity::Keypair, upgrade},
    PeerId, Multiaddr, multiaddr::Protocol, tcp, Transport, mplex::MplexConfig, noise
};
pub use libipld::cid::Cid;
pub use futures::{
    future::{BoxFuture, join_all, Either},
    stream::{FuturesUnordered, StreamExt, Stream},
};
pub use reqwest::Client;
pub use sha2_derive::Hashable;
pub use libp2p_identify::{Behaviour as IdentifyBehaviour, Event as IdentifyEvent, Config as IdentifyConfig};
pub use word_lists::HackTraitSortedContains;

pub type SearchController = OngoingSearchController<125000, DocumentIndex<125000>>;

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("Invalid system time").as_secs()
}
