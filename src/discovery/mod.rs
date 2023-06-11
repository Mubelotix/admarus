//! This module contains a simple protocol for peer discovery.

pub use futures::{
    future::{self, BoxFuture},
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
};
pub use libp2p::{
    core::{muxing::SubstreamBox, Negotiated, UpgradeInfo},
    swarm::{
        handler::ConnectionEvent, ConnectionHandler, ConnectionHandlerEvent, ConnectionId,
        FromSwarm, KeepAlive, NetworkBehaviour, PollParameters, SubstreamProtocol, THandlerInEvent,
        THandlerOutEvent, ToSwarm,
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
pub use tokio::sync::RwLock;

mod behavior;
mod client_server;
mod handler;
mod protocol;
mod db;
mod config;

pub use {behavior::*, client_server::*, handler::*, protocol::*, db::*, config::*};

pub type Stream = Negotiated<SubstreamBox>;
