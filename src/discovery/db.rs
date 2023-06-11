
use super::*;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    ///The public key of the local peer.
    //pub public_key: PublicKey,

    /// Application-specific version of the protocol family used by the peer, e.g. ipfs/1.0.0 or polkadot/1.0.0.
    pub protocol_version: String,

    /// Name and version of the peer, similar to the User-Agent header in the HTTP protocol.
    pub agent_version: String,

    /// The addresses that the peer is listening on.
    pub listen_addrs: Vec<Multiaddr>,

    /// The list of protocols supported by the peer, e.g. /ipfs/ping/1.0.0.
    pub protocols: Vec<String>,

    /// Address observed by or for the remote.
    pub observed_addr: Option<Multiaddr>,

    /// Arbitrary metadata provided by the peer.
    pub metadata: Vec<u8>,
}

pub struct Db {
    config: Arc<Config>,
    data: RwLock<HashMap<PeerId, (bool, Info)>>,
}

impl Db {
    pub fn config(&self) -> &Config {
        &self.config
    }

    pub async fn remove_peer(&self, peer_id: &PeerId) {
        self.data.write().await.remove(peer_id);
    }

    pub async fn insert_peer(&self, peer_id: PeerId) {
        let mut data = self.data.write().await;
        data.entry(peer_id).or_insert((self.config.default_visibility, Info::default()));
    }

    pub async fn set_visibility(&self, peer_id: &PeerId, visible: bool) {
        self.data.write().await.entry(*peer_id).or_insert((self.config.default_visibility, Info::default())).0 = visible;
    }

    pub async fn set_info(&self, peer_id: &PeerId, info: Info) {
        self.data.write().await.entry(*peer_id).or_insert((self.config.default_visibility, Info::default())).1 = info;
    }

    pub async fn set_metadata(&self, peer_id: &PeerId, metadata: Vec<u8>) {
        self.data.write().await.entry(*peer_id).or_insert((self.config.default_visibility, Info::default())).1.metadata = metadata;
    }

    pub async fn gen_list(
        &self,
        protocol_version: Option<String>,
        agent_version: Option<String>,
        protocols: Option<Vec<String>>,
        metadata: Option<Vec<u8>>
    ) -> Vec<(PeerId, Info)> {
        self.data.read().await
            .iter()
            .filter(|(_, (visible, _))| *visible)
            .filter(|(_, (_, info))| match protocol_version {
                Some(ref protocol_version) => info.protocol_version == *protocol_version,
                None => true,
            })
            .filter(|(_, (_, info))| match agent_version {
                Some(ref agent_version) => info.agent_version == *agent_version,
                None => true,
            })
            .filter(|(_, (_, info))| match protocols {
                Some(ref protocols) => protocols.iter().all(|protocol| info.protocols.contains(protocol)),
                None => true,
            })
            .filter(|(_, (_, info))| match metadata {
                Some(ref metadata) => info.metadata == *metadata,
                None => true,
            })
            .map(|(peer_id, (_, info))| (*peer_id, info.clone()))
            .collect()
    }
}
