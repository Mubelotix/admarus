//! This module contains a simple protocol for peer discovery.
#![allow(dead_code)]

pub use futures::{
    future::{self, BoxFuture},
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
};
pub use libp2p::{
    core::{muxing::SubstreamBox, Negotiated, UpgradeInfo},
    swarm::{
        handler::ConnectionEvent, ConnectionHandler, ConnectionHandlerEvent, ConnectionId,
        FromSwarm, KeepAlive, NetworkBehaviour, PollParameters, SubstreamProtocol, THandlerInEvent,
        THandlerOutEvent, ToSwarm, NotifyHandler, Stream,
    },
    InboundUpgrade, Multiaddr, OutboundUpgrade, PeerId,
};
pub use log::{debug, error, info, trace, warn};
pub use serde::{Deserialize, Serialize};
pub use std::{
    collections::HashMap,
    sync::Arc,
    task::{Context, Poll},
    io::Error as IoError,
};
pub use tokio::sync::{RwLock, oneshot::{Sender as OneshotSender, Receiver as OneshotReceiver, channel as oneshot_channel}};

mod behavior;
mod client_server;
mod handler;
mod protocol;
mod db;
mod config;
mod query;

pub use {behavior::*, client_server::*, handler::*, protocol::*, db::*, config::*, query::*};

pub type RequestReplier = OneshotSender<Result<Response, IoError>>;
