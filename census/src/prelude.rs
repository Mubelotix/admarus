pub(crate) use tokio::{sync::RwLock, time::sleep};
pub(crate) use std::{time::{Duration, SystemTime, UNIX_EPOCH}, collections::HashSet, pin::Pin, future::Future};
pub use crate::{db::*, record::*, endpoints::*};
pub(crate) use serde::{Serialize, Deserialize};
pub(crate) use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder, HttpRequest};
pub(crate) use rand::seq::IteratorRandom;
pub(crate) use sha2_derive::Hashable;
pub(crate) use libp2p::core::identity::PublicKey;

pub(crate) fn now_ts() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() }
