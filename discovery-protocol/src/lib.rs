//! This module contains a simple protocol for peer discovery.
#![allow(dead_code)]

pub(crate) use futures::{
    future::{self, BoxFuture},
    io::{AsyncReadExt, AsyncWriteExt},
};
pub(crate) use libp2p::{
    swarm::{
        handler::ConnectionEvent, ConnectionHandler, ConnectionHandlerEvent, ConnectionId,
        FromSwarm, NetworkBehaviour, SubstreamProtocol, THandlerInEvent,
        THandlerOutEvent, ToSwarm, NotifyHandler, Stream,
    },
    core::UpgradeInfo, InboundUpgrade, Multiaddr, OutboundUpgrade, PeerId,
};
pub(crate) use log::{debug, error};
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use std::{
    collections::HashMap,
    sync::Arc,
    task::{Context, Poll},
    io::Error as IoError,
    mem::drop
};
pub(crate) use tokio::sync::{RwLock, oneshot::{Sender as OneshotSender, channel as oneshot_channel}};

mod behavior;
mod client_server;
mod handler;
mod protocol;
mod db;
mod config;
mod query;

pub use {behavior::*, client_server::*, handler::*, protocol::*, db::*, config::*, query::*};

pub(crate) type RequestReplier = OneshotSender<Result<Response, IoError>>;
