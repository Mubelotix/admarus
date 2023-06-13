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


struct PeerInfo {
    addrs: Vec<Multiaddr>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PeerClass {
    First,
    Second,
    Transient
}

impl ConnectedPeerInfo {
    pub fn role(&self) -> PeerClass {
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
        self.connected_peers.read().await.values().for_each(|i| match i.role() {
            PeerClass::First => first_class_count += 1,
            PeerClass::Second => second_class_count += 1,
            PeerClass::Transient => transient_count += 1,
        });
        (first_class_count, second_class_count, transient_count)
    }

    pub async fn first_class_slot_available(&self) -> bool {
        let seeder_count = self.class_counts().await.0;
        seeder_count < self.config.seeders
    }

    pub async fn second_class_slot_available(&self) -> bool {
        let leecher_count = self.class_counts().await.1;
        leecher_count < self.config.leechers
    }

    pub async fn class(&self, peer_id: &PeerId) -> Option<PeerClass> {
        self.connected_peers.read().await.get(peer_id).map(|i| i.role())
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
        let leecher_count = connected_peers.values().filter(|i| i.role() == PeerClass::Second).count();
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

pub async fn manage_swarm(controller: KamilataController) {

}
