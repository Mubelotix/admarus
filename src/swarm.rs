//! # Peer swarm managment 
//! 
//! This implementation attributes slots to different kind of peers.
//! The policies are strongly enforced, and the swarm isn't reluctant to disconnect peers.
//! 
//! ## First-class peers
//! 
//! Those are select peers we chose to leech from.
//! We chose those whom we trust the most.
//! We try to reach SEEDER_TARGET, and we never go above.
//! These peers have guaranteed slots as leechers too.
//! = SEEDER_TARGET
//! 
//! ## Second-class peers
//! 
//! Those are peers who selected us as first-class peers.
//! We leech back from all leechers, though they don't count as seeders.
//! Leechers have the right to refuse to seed us.
//! When new peers apply for a leecher slot and they are all taken, we disconnect the peer with the lowest score.
//! In order to prevent a malicious actor from replacing all legitimate leechers, peers that cause a disconnection start with a reputation malus.
//! <= MAX_LEECHERS
//! 
//! ## Transient peers
//! 
//! Some peers connect for a few seconds, the time to send us queries. We do the same to them.
//! Those peers are theoretically unlimited, but there is a practical high limit at MAX_FAST_PACED_SLOTS.
//! The main limit is actually the time those peers are allowed to stay connected.
//! When that time is up, we disconnect them. We might be more tolerant when we have plenty of slots available.
//! <= MAX_FAST_PACED_SLOTS

use crate::prelude::*;

struct ConnectedPeerInfo {
    selected: bool,
    seeding: bool,
    leeching: bool,
    connected_since: Instant,
}

#[derive(Clone, Default)]
pub struct PeerInfo {
    addrs: Vec<Multiaddr>,
    score: f32,
    recommander_score: f32,

    last_seen_ipfs: Option<Instant>,
    last_seen: Option<Instant>,
    recommended_by: Vec<(PeerId, Instant)>,
}

impl PeerInfo {
    pub fn last_updated(&self) -> Option<Instant> {
        let mut latest = self.last_seen_ipfs;
        if self.last_seen > latest {
            latest = self.last_seen;
        }
        for (_, time) in self.recommended_by.iter() {
            if Some(*time) > latest {
                latest = Some(*time);
            }
        }

        latest
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PeerClass {
    First,
    Second,
    Transient
}

impl ConnectedPeerInfo {
    pub fn class(&self) -> PeerClass {
        if self.selected {
            PeerClass::First
        } else if self.leeching {
            PeerClass::Second
        } else {
            PeerClass::Transient
        }
    }
}

pub struct SwarmManager {
    config: Arc<Args>,
    known_peers: RwLock<HashMap<PeerId, PeerInfo>>,

    connected_peers: RwLock<HashMap<PeerId, ConnectedPeerInfo>>,
}

impl SwarmManager {
    pub fn new(config: Arc<Args>) -> SwarmManager {
        SwarmManager {
            config,
            known_peers: RwLock::new(HashMap::new()),
            connected_peers: RwLock::new(HashMap::new()),
        }
    }
}

impl SwarmManager {
    pub async fn class_counts(&self) -> (usize, usize, usize) {
        let mut first_class_count = 0;
        let mut second_class_count = 0;
        let mut transient_count = 0;
        self.connected_peers.read().await.values().for_each(|i| match i.class() {
            PeerClass::First => first_class_count += 1,
            PeerClass::Second => second_class_count += 1,
            PeerClass::Transient => transient_count += 1,
        });
        (first_class_count, second_class_count, transient_count)
    }

    pub async fn first_class_slot_available(&self) -> bool {
        let seeder_count = self.class_counts().await.0;
        seeder_count < self.config.first_class
    }

    pub async fn second_class_slot_available(&self) -> bool {
        let leecher_count = self.class_counts().await.1;
        leecher_count < self.config.leechers
    }

    pub async fn class(&self, peer_id: &PeerId) -> Option<PeerClass> {
        self.connected_peers.read().await.get(peer_id).map(|i| i.class())
    }

    pub async fn on_peer_connected(&self, peer_id: PeerId) {
        let mut connected_peers = self.connected_peers.write().await;
        connected_peers.insert(peer_id, ConnectedPeerInfo {
            selected: false,
            seeding: false,
            leeching: false,
            connected_since: Instant::now(),
        });
    }

    pub async fn on_peer_disconnected(&self, peer_id: &PeerId) {
        let mut connected_peers = self.connected_peers.write().await;
        connected_peers.remove(peer_id);
    }

    pub async fn on_seeder_added(&self, peer_id: PeerId) {
        let mut connected_peers = self.connected_peers.write().await;
        connected_peers.entry(peer_id).and_modify(|i| i.seeding = true);
    }

    pub async fn on_leecher_added(&self, peer_id: PeerId) -> Result<(), TooManyLeechers> {
        let mut connected_peers = self.connected_peers.write().await;
        connected_peers.entry(peer_id).and_modify(|i| i.leeching = true);
        let leecher_count = connected_peers.values().filter(|i| i.class() == PeerClass::Second).count();
        if leecher_count > self.config.leechers {
            return Err(TooManyLeechers{})
        }
        Ok(())
    }

    pub async fn on_seeder_removed(&self, peer_id: &PeerId) {
        let mut connected_peers = self.connected_peers.write().await;
        connected_peers.entry(*peer_id).and_modify(|i| i.seeding = false);
    }

    pub async fn on_leecher_removed(&self, peer_id: &PeerId) {
        let mut connected_peers = self.connected_peers.write().await;
        connected_peers.entry(*peer_id).and_modify(|i| i.leeching = false);
    }
}

/// Some of our ipfs peers might run Admarus.
/// We try to connect randomly on the default Admarus port (4002).
pub async fn bootstrap_from_ipfs(controller: KamilataController, config: Arc<Args>) {
    loop {
        let peers = match get_ipfs_peers(&config.ipfs_rpc).await {
            Ok(peers) => peers,
            Err(e) => {
                error!("Failed to bootstrap from ipfs peers: {e:?}");
                return;
            }
        };
    
        let now = Instant::now();
        let mut known_peers = controller.sw.known_peers.write().await;
        let previous_len = known_peers.len();
        for (peer_id, addr) in peers {
            let known_peer = known_peers.entry(peer_id).or_default();
            known_peer.addrs.push(addr);
            known_peer.last_seen_ipfs = Some(now);
        }
        let new_len = known_peers.len();
        drop(known_peers);
        if new_len != previous_len {
            debug!("Added {} new peers from ipfs", new_len - previous_len);
        }

        sleep(Duration::from_secs(5*60)).await;
    }
}

pub async fn cleanup_known_peers(controller: KamilataController) {
    loop {
        let mut known_peers = controller.sw.known_peers.write().await;
        let previous_len = known_peers.len();
        known_peers.retain(|_, info| {
            match info.last_updated() {
                Some(last_updated) => last_updated.elapsed() < Duration::from_secs(7*86400),
                None => false,
            }
        });
        let new_len = known_peers.len();
        drop(known_peers);
        if new_len != previous_len {
            debug!("Removed {} peers from known peers database (outdated data)", previous_len - new_len);
        }

        sleep(Duration::from_secs(60*60)).await;
    }
}

pub async fn manage_swarm(controller: KamilataController, config: Arc<Args>) {
    let sw = Arc::clone(&controller.sw);

    let mut dial_attemps: HashMap<PeerId, Instant> = HashMap::new();

    loop {
        // Unselect all first-class peers that are not seeding
        // Disconnect all transient peers that are staying too long
        for (peer_id, info) in sw.connected_peers.write().await.iter_mut() {
            match info.class() {
                PeerClass::First => if !info.seeding && info.connected_since.elapsed() > Duration::from_secs(60) {
                    info.selected = false;
                    // TODO visility in discovery
                },
                PeerClass::Transient => if info.connected_since.elapsed() > Duration::from_secs(60) {
                    debug!("Disconnecting transient peer {peer_id}");
                    controller.disconnect(peer_id).await;
                },
                _ => (),
            }
        }

        // Sweep dial_attemps
        dial_attemps.retain(|_,time| time.elapsed() < Duration::from_secs(3600));
        let currently_dialing = dial_attemps.values().filter(|t| t.elapsed() < Duration::from_secs(100)).count();

        // Looking for more peers
        let (fcp_count, _scp_count, _tp_count) = sw.class_counts().await;
        let missing_fcp = config.first_class.saturating_sub(fcp_count).saturating_sub(currently_dialing);
        if missing_fcp == 0 { continue }
        debug!("Not enough first-class peers, looking for {} more", missing_fcp);

        {
            let known_peers = sw.known_peers.read().await;
            let connected_peers = sw.connected_peers.read().await;

            let mut candidates = known_peers
                .iter()
                .filter(|(peer_id, _)| 
                    !connected_peers.get(peer_id).map(|i| i.selected).unwrap_or(false)
                    && !dial_attemps.contains_key(peer_id)
                )
                .collect::<Vec<_>>();
            candidates.sort_by(|(_, a), (_, b)| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
            candidates.truncate(missing_fcp);
            drop(connected_peers);
            
            let mut connected_peers = sw.connected_peers.write().await;
            for (peer_id, info) in candidates {
                if let Some(connected_info) = connected_peers.get_mut(peer_id) {
                    debug!("Selecting first-class peer {peer_id}");
                    connected_info.selected = true;
                } else {
                    debug!("Dialing new peer {peer_id}");
                    controller.dial_with_peer_id(*peer_id, info.addrs.clone()).await;
                    dial_attemps.insert(*peer_id, Instant::now());
                }
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}
