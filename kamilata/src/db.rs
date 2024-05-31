use std::collections::BTreeSet;
use crate::prelude::*;

pub(crate) struct Db<const N: usize, S: Store<N>> {
    // In order to prevent deadlocks, please lock the different fields in the same order as they are declared in the struct.

    config: Arc<KamilataConfig>,
    behaviour_controller: BehaviourController<N, S>,
    /// Documents to add in the global network corpus
    store: S,
    /// Filters received from seeders
    seeder_filters: RwLock<BTreeMap<PeerId, Vec<Filter<N>>>>,
    /// Peers we send filters to
    leechers: RwLock<BTreeSet<PeerId>>,
    /// Known addresses of peers that are connected to us
    addrs: RwLock<BTreeMap<PeerId, Vec<Multiaddr>>>,
}

impl<const N: usize, S: Store<N>> Db<N, S> {
    pub fn new(config: Arc<KamilataConfig>, store: S, behaviour_controller: BehaviourController<N, S>) -> Self {
        Db {
            config,
            behaviour_controller,
            store,
            seeder_filters: RwLock::new(BTreeMap::new()),
            addrs: RwLock::new(BTreeMap::new()),
            leechers: RwLock::new(BTreeSet::new()),
        }
    }

    pub fn get_config(&self) -> Arc<KamilataConfig> {
        Arc::clone(&self.config)
    }

    pub(crate) fn behaviour_controller(&self) -> &BehaviourController<N, S> {
        &self.behaviour_controller
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub async fn seeder_count(&self) -> usize {
        self.seeder_filters.read().await.len()
    }

    pub async fn leecher_count(&self) -> usize {
        self.leechers.read().await.len()
    }

    /// Adds a new connected peer.
    pub async fn add_peer(&self, peer_id: PeerId, addrs: Vec<Multiaddr>) {
        self.addrs.write().await.insert(peer_id, addrs);
    }

    /// Remove data about a peer.
    pub async fn remove_peer(&self, peer_id: &PeerId) {
        self.seeder_filters.write().await.remove(peer_id);
        self.addrs.write().await.remove(peer_id);
        self.leechers.write().await.remove(peer_id);
    }

    /// Claims a spot as a leecher.
    pub async fn add_leecher(&self, peer_id: PeerId) -> Result<(), TooManyLeechers> {
        let mut leachers = self.leechers.write().await;
        if leachers.len() < self.config.max_leechers {
            leachers.insert(peer_id);
            Ok(())
        } else {
            Err(TooManyLeechers{})
        }
    }

    /// Claims a spot as a seeder.
    pub async fn add_seeder(&self, peer_id: PeerId) -> Result<(), TooManySeeders> {
        let mut seeder_filters = self.seeder_filters.write().await;
        if seeder_filters.len() < self.config.max_seeders {
            seeder_filters.insert(peer_id, Vec::new());
            Ok(())
        } else {
            Err(TooManySeeders{})
        }
    }

    pub async fn set_remote_filter(&self, peer_id: PeerId, filters: Vec<Filter<N>>) {
        // TODO size checks
        self.seeder_filters.write().await.insert(peer_id, filters);
    }

    pub(crate) async fn get_filters(&self, ignore_peers: &[PeerId]) -> Vec<Filter<N>> {
        let mut result = Vec::new();
        result.push(self.store.get_filter().await); // FIXME: This is slow

        let filters = self.seeder_filters.read().await;
        for level in 1..10 {
            let mut filter = Filter::new();
            let mut is_null = true;
            for (peer_id, filters) in filters.iter() {
                if ignore_peers.contains(peer_id) {
                    continue;
                }
                if let Some(f) = filters.get(level-1) {
                    filter.bitor_assign_ref(f);
                    is_null = false;
                }
            }
            match is_null {
                true => break,
                false => result.push(filter),
            }
        }

        result
    }

    pub(crate) async fn get_filters_bytes(&self, ignore_peers: &[PeerId]) -> Vec<Vec<u8>> {
        let filters = self.get_filters(ignore_peers).await;
        filters.into_iter().map(|f| <Vec<u8>>::from(&f)).collect()
    }

    /// Adds a new address for a peer.
    pub async fn add_address(&self, peer_id: PeerId, addr: Multiaddr, front: bool) -> Result<(), DisconnectedPeer> {
        let mut addrs = self.addrs.write().await;
        let addrs = addrs.get_mut(&peer_id).ok_or(DisconnectedPeer)?;
        if !addrs.contains(&addr) {
            match front {
                true => addrs.insert(0, addr),
                false => addrs.push(addr),
            }
        }
        Ok(())
    }

    /// Sets the addresses for a peer.
    pub async fn set_addresses(&self, peer_id: PeerId, addrs: Vec<Multiaddr>) -> Result<(), DisconnectedPeer> {
        let mut all_addrs = self.addrs.write().await;
        if !all_addrs.contains_key(&peer_id) {
            return Err(DisconnectedPeer);
        }
        all_addrs.insert(peer_id, addrs);
        Ok(())
    }

    /// Gets the addresses we know for a peer, ordered by how well they are expected to work.
    pub async fn get_addresses(&self, peer_id: &PeerId) -> Vec<Multiaddr> {
        self.addrs.read().await.get(peer_id).cloned().unwrap_or_default()
    }

    /// Returns peers and their distance to each query.
    /// Each peer is tested for all its filters, and the matching priorities are returned in an array.
    pub async fn search_routes(&self, query: &S::Query) -> Vec<(PeerId, Vec<u32>)> {
        let filters = self.seeder_filters.read().await;
        filters
            .iter()
            .map(|(peer_id, filters)| {
                (*peer_id, filters.iter().map(|f| query.match_score(f)).collect::<Vec<_>>())
            })
            .filter(|(_,m)| m.iter().any(|d| *d > 0))
            .collect()
    }

    /*
    
    /// Returns peers and their distance to the query.
    pub async fn search_routes(&self, hashed_words: Vec<usize>, min_matching: usize) -> Vec<(PeerId, usize)> {
        let filters = self.filters.read().await;
        filters
            .iter()
            .filter_map(|(peer_id, filters)|
                filters.iter().position(|f| {
                    hashed_words.iter().filter(|w| f.get_bit(**w)).count() >= min_matching
                })
                .map(|d| (*peer_id, d))
            )
            .collect()
    }

     */
}

/// Error returned when we try to add a new leecher but there are already too many.
#[derive(Debug, Clone)]
pub struct TooManyLeechers {}

/// Error returned when we try to add a new seeder but there are already too many.
#[derive(Debug, Clone)]
pub struct TooManySeeders {}

/// Error returned when we try an operation on a peer that is not connected to us.
#[derive(Debug, Clone)]
pub struct DisconnectedPeer;
