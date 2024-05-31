use crate::prelude::*;

#[derive(Debug, Clone, protocol::Protocol)]
pub struct MinTargetMax {
    pub(crate) min: u64,
    pub(crate) target: u64,
    pub(crate) max: u64,
}

impl MinTargetMax {
    pub const fn new(min: usize, target: usize, max: usize) -> Self {
        // TODO checks
        Self {
            min: min as u64,
            target: target as u64,
            max: max as u64,
        }
    }

    pub fn set_min(&mut self, min: usize) {
        self.min = min as u64;
        if self.target < self.min {
            self.target = self.min;
        }
        if self.max < self.min {
            self.max = self.min;
        }
    }

    pub fn min(&self) -> usize {
        self.min as usize
    }

    pub fn set_max(&mut self, max: usize) {
        self.max = max as u64;
        if self.target > self.max {
            self.target = self.max;
        }
        if self.min > self.max {
            self.min = self.max;
        }
    }

    pub fn max(&self) -> usize {
        self.max as usize
    }

    pub fn set_target(&mut self, target: usize) {
        self.target = target as u64;
        if self.min > self.target {
            self.min = self.target;
        }
        if self.max < self.target {
            self.max = self.target;
        }
    }

    pub fn target(&self) -> usize {
        self.target as usize
    }

    pub fn intersection(&self, other: &Self) -> Option<Self> {
        let min = self.min.max(other.min);
        let max = self.max.min(other.max);
        if max < min {
            return None;
        }
        let target = ((self.target + other.target) / 2).clamp(min, max);
        Some(Self { min, target, max })
    }

    pub fn is_under_target(&self, value: usize) -> bool {
        let value = value as u64;
        value < self.target
    }

    pub fn is_max_or_over(&self, value: usize) -> bool {
        let value = value as u64;
        value >= self.max
    }
}

pub type ApprocheLeecherClosure = Box<dyn (Fn(PeerId) -> Pin<Box<dyn std::future::Future<Output = bool> + Send>>) + Sync + Send>;

pub struct KamilataConfig {
    /// Custom protocol names
    /// 
    /// Kamilata nodes only communicate with other nodes using the same protocol name.
    /// Using custom name(s) therefore allows to segregate nodes from others, if that is desired.
    /// 
    /// More than one protocol name can be supplied.
    /// In this case the node will be able to talk to other nodes supporting any of the provided names.
    /// Multiple names must be used with caution to avoid network partitioning.
    pub protocol_names: Vec<String>,
    /// Min, target and max values in milliseconds
    pub get_filters_interval: MinTargetMax,
    /// Maximum number of filters to manage per peer (default: 8)
    pub filter_count: usize,
    /// Maximum number of peers we receive filters from (default: 20)
    pub max_seeders: usize,
    /// Maximum number of peers we send filters to (default: 50)
    pub max_leechers: usize,
    /// This closure is called when a peer wants to leech from us.
    /// If it returns true, the peer is allowed to leech.
    /// If this closure is not set, all peers are allowed to leech.
    /// 
    /// Note that the `max_leechers` limit is always enforced.
    /// As a result, a peer might be rejected even after this closure returns true.
    /// 
    /// # Example
    /// 
    /// ```
    /// # use {std::{pin::Pin, future::Future}, libp2p::PeerId, kamilata::config::*};
    /// fn approve_leecher(peer_id: PeerId) -> Pin<Box<dyn Future<Output = bool> + Send>> {
    ///     Box::pin(async move {
    ///        // TODO
    /// #      true
    ///     })
    /// }
    /// # let t: ApprocheLeecherClosure = Box::new(approve_leecher);
    /// ```
    pub approve_leecher: Option<ApprocheLeecherClosure>,
}

impl std::fmt::Debug for KamilataConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KamilataConfig")
            .field("protocol_names", &self.protocol_names)
            .field("get_filters_interval", &self.get_filters_interval)
            .field("filter_count", &self.filter_count)
            .field("max_seeders", &self.max_seeders)
            .field("max_leechers", &self.max_leechers)
            .field("is_approved_leecher", match self.approve_leecher.is_some() {
                true => &"Some([closure])",
                false => &"None",
            })
            .finish()
    }
}

impl Default for KamilataConfig {
    fn default() -> Self {
        Self {
            protocol_names: vec![String::from("/kamilata/1.0.0")],
            get_filters_interval: MinTargetMax { min: 15_000, target: 20_000, max: 60_000*3 },
            filter_count: 8,
            max_seeders: 20,
            max_leechers: 50,
            approve_leecher: None,
        }
    }
}
