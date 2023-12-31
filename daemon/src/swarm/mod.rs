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
//! 
//! # Reputation and score system
//! 
//! See the following list of events and their impact on the reputation score:
//! 
//! - Returned result is upvoted by the user: +5000
//! - Returned result is automatically upvoted: +500
//! - Returns search results: +100
//! - Correct dns link: +20
//! - Routes to a peer that returns search results: +10
//! - Successful dial: +10
//! - Returns peers for discovery: +5
//! - First-class peer sent us filters: +0.2
//! - Other peer sent us filters: +0.1
//! - Asks for our peers: -2
//! - Queries us: -5
//! - Disconnects: -5
//! - Incorrect dns link: -100
//! - Returned result is automatically downvoted: -500
//! - Returned result is downvoted by the user: -5000
//! - Lies on document content: banned

use crate::prelude::*;

mod maintain_swarm;
mod cleanup_db;
mod update_census;
mod get_peers;
pub use {maintain_swarm::*, cleanup_db::*, update_census::*, get_peers::*};

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

    successful_dials: u64,
    failed_dials: u64,

    last_seen_ipfs: Option<u64>,
    last_seen: Option<u64>,
    last_returned_by_census: Option<u64>,
    recommended_by: HashMap<PeerId, u64>,
}

impl PeerInfo {
    pub fn last_updated(&self) -> Option<u64> {
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

    pub fn availability(&self) -> f64 {
        self.successful_dials as f64 / (self.successful_dials + self.failed_dials) as f64
    }

    pub fn source_reliability(&self) -> usize {
        let mut reliability = 0;
        if self.last_seen_ipfs.is_some() {
            reliability += 1;
        }
        if self.last_returned_by_census.is_some() {
            reliability += 10;
        }
        if self.last_seen.is_some() {
            reliability += 100;
        }
        reliability += std::cmp::min(self.recommended_by.len() * 5, 90);
        reliability
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
    dial_attemps: RwLock<HashMap<PeerId, Instant>>,
    connected_peers: RwLock<HashMap<PeerId, ConnectedPeerInfo>>,
}

impl SwarmManager {
    pub fn new(config: Arc<Args>) -> SwarmManager {
        SwarmManager {
            config,
            known_peers: RwLock::new(HashMap::new()),
            dial_attemps: RwLock::new(HashMap::new()),
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

    pub async fn second_class_slot_available(&self) -> bool {
        let leecher_count = self.class_counts().await.1;
        leecher_count < self.config.leechers
    }

    pub async fn class(&self, peer_id: &PeerId) -> Option<PeerClass> {
        self.connected_peers.read().await.get(peer_id).map(|i| i.class())
    }

    pub async fn sweep_dial_attempts(&self) {
        let mut dial_attemps = self.dial_attemps.write().await;
        dial_attemps.retain(|_,time| time.elapsed() < Duration::from_secs(3600));
    }

    pub async fn currently_dialing(&self) -> usize {
        let dial_attemps = self.dial_attemps.read().await;
        dial_attemps.values().filter(|t| t.elapsed() < Duration::from_secs(30)).count()
    }

    pub async fn get_peers_to_dial(&self, count: usize) -> Vec<(PeerId, PeerInfo)> {
        let known_peers = self.known_peers.read().await;
        let dial_attempts = self.dial_attemps.read().await;
        let connected_peers = self.connected_peers.read().await;

        let mut candidates = known_peers
            .iter()
            .filter(|(peer_id, _)| 
                !connected_peers.get(peer_id).map(|i| i.selected).unwrap_or(false)
                && (!dial_attempts.contains_key(peer_id) || connected_peers.contains_key(peer_id))
            )
            .collect::<Vec<_>>();

        candidates.sort_by(|(aid, a), (bid, b)| {
            let mut ordering = b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal);
            if ordering == Ordering::Equal {
                ordering = connected_peers.contains_key(bid).cmp(&connected_peers.contains_key(aid));
            }
            if ordering == Ordering::Equal {
                ordering = b.availability().partial_cmp(&a.availability()).unwrap_or(Ordering::Equal);
            }
            if ordering == Ordering::Equal {
                ordering = b.source_reliability().partial_cmp(&a.source_reliability()).unwrap_or(Ordering::Equal);
            }
            ordering
        });

        candidates.truncate(count);

        candidates.into_iter().map(|(a,b)| (*a,b.clone())).collect()
    }

    pub async fn on_peer_connected(&self, peer_id: PeerId) {
        let mut known_peers = self.known_peers.write().await;
        let dial_attempts = self.dial_attemps.read().await;
        let mut connected_peers = self.connected_peers.write().await;
        connected_peers.entry(peer_id).or_insert(ConnectedPeerInfo {
            selected: false,
            seeding: false,
            leeching: false,
            connected_since: Instant::now(),
        });
        let mut peer_info = known_peers.entry(peer_id).or_default();
        peer_info.last_seen = Some(now());
        if dial_attempts.contains_key(&peer_id) {
            if peer_info.failed_dials == 0 {
                warn!("Peer {peer_id} connected after being dialed, but the dial attempt was not counted");
            }
            peer_info.failed_dials = peer_info.failed_dials.saturating_sub(1);
            peer_info.successful_dials += 1;
        }
    }

    pub async fn on_peer_disconnected(&self, peer_id: &PeerId) {
        let mut known_peers = self.known_peers.write().await;
        let mut connected_peers = self.connected_peers.write().await;
        connected_peers.remove(peer_id);
        let mut peer_info = known_peers.entry(*peer_id).or_default();
        peer_info.last_seen = Some(now());
    }

    pub async fn on_identify(&self, peer_id: &PeerId, info: libp2p_identify::Info) {
        let mut known_peers = self.known_peers.write().await;
        let mut peer_info = known_peers.entry(*peer_id).or_default();
        peer_info.addrs = info.listen_addrs;
        // TODO: other fields
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


