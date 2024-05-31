use protocol_derive::Protocol;
use crate::config::MinTargetMax;

#[derive(Clone)]
#[repr(transparent)]
pub struct PeerId(libp2p::PeerId);

impl From<libp2p::PeerId> for PeerId {
    fn from(peer_id: libp2p::PeerId) -> Self {
        PeerId(peer_id)
    }
}

impl From<PeerId> for libp2p::PeerId {
    fn from(peer_id: PeerId) -> Self {
        peer_id.0
    }
}

impl protocol::Parcel for PeerId {
    const TYPE_NAME: &'static str = "PeerId";

    fn read_field(read: &mut dyn std::io::Read,
                  settings: &protocol::Settings,
                  hints: &mut protocol::hint::Hints) -> Result<Self, protocol::Error> {
        let lenght: u16 = protocol::Parcel::read_field(read, settings, hints)?;
        let mut bytes = vec![0; lenght as usize];
        read.read_exact(&mut bytes)?;
        match libp2p::PeerId::from_bytes(&bytes) {
            Ok(peer_id) => Ok(PeerId(peer_id)),
            Err(_) => Err(protocol::Error::from_kind(protocol::ErrorKind::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid peer id")))),
        }
    }

    fn write_field(&self, write: &mut dyn std::io::Write,
             settings: &protocol::Settings,
             hints: &mut protocol::hint::Hints) -> Result<(), protocol::Error> {
        let bytes = self.0.to_bytes();
        let lenght = bytes.len() as u16;
        lenght.write_field(write, settings, hints)?;
        write.write_all(&bytes)?;
        Ok(())
    }
}

impl std::fmt::Debug for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

pub trait HackTraitVecPeerId {
    fn to_libp2p_peer_ids(self) -> Vec<libp2p::PeerId>;
}
impl HackTraitVecPeerId for Vec<PeerId> {
    fn to_libp2p_peer_ids(self) -> Vec<libp2p::PeerId> {
        unsafe {
            std::mem::transmute_copy(&self)
        }
    }
}

#[derive(Protocol, Debug, Clone)]
pub enum RequestPacket {
    /// Request the peer to send us its filters.
    /// The peer accepts by continuously sending [ResponsePacket::UpdateFilters], or closes the channel.
    GetFilters(GetFiltersPacket),
    /// Asks to apply our query on its documents and return results in the [ResponsePacket::ReturnResults] packet.
    Search(SearchPacket),

    Disconnect(DisconnectPacket),
}

#[derive(Protocol, Debug, Clone)]
pub struct GetFiltersPacket {
    /// Number of filters to send, from level-0 filter to level-`filter_count-1` filter
    pub filter_count: u8,
    /// Milliseconds between each update
    pub interval: MinTargetMax,
    /// Peers we don't want to hear from
    pub blocked_peers: Vec<PeerId>,
}

impl Default for GetFiltersPacket {
    fn default() -> Self {
        GetFiltersPacket {
            filter_count: 6,
            interval: MinTargetMax {
                min: 15_000,
                target: 21_000,
                max: 60_000,
            },
            blocked_peers: Vec::new(),
        }
    }
}

/// For a filter to match a query, it must have at least `match_count` bits set to 1 at the positions specified by hashed `words`.
#[derive(Protocol, Debug, Clone)]
pub struct Query {
    /// List of words to search for.
    pub words: Vec<String>,
    /// Minimum number of words that must match in order for a filter to match the query.
    /// Invalid if greater than `words.len()`.
    pub min_matching: u16,
}

#[derive(Protocol, Debug, Clone)]
pub struct SearchPacket {
    /// A query that will be decoded with [SearchQuery::from_bytes].
    pub query: Vec<u8>,
}

#[derive(Protocol, Debug, Clone)]
pub enum ResponsePacket {
    /// Sent periodically to inform the peers of our filters.
    UpdateFilters(UpdateFiltersPacket),
    /// Response to a [RequestPacket::Search] packet.
    /// Will be followed by multiple [ResponsePacket::Result].
    Routes(RoutesPacket),
    /// Response to a [RequestPacket::Search] packet.
    Result(ResultPacket),
    /// Sent once all [ResponsePacket::Result] have been sent.
    SearchOver,

    Disconnect(DisconnectPacket),
}

#[derive(Protocol, Debug, Clone)]
pub struct UpdateFiltersPacket {
    /// The filters ordered from distance 0 to the furthest at a distance of [RefreshPacket::range].
    pub filters: Vec<Vec<u8>>,
}

#[derive(Protocol, Debug, Clone)]
pub struct Route {
    /// An array of match scores for each filter of the peer.
    /// At least one of the items in this list should be non-zero.
    pub match_scores: Vec<u32>,
    pub peer_id: PeerId,
    pub addresses: Vec<String>,
}

#[derive(Protocol, Debug, Clone)]
pub struct RoutesPacket(
    /// A list of routing information to be used to find actual results.
    pub Vec<Route>
);

#[derive(Protocol, Debug, Clone)]
pub struct ResultPacket(
    /// Contains a result to be deserialized and used.
    pub Vec<u8>,
);

#[derive(Protocol, Debug, Clone)]
pub struct DisconnectPacket {
    /// The reason for the disconnection.
    pub reason: String,
    /// Asks the peer to reconnect after a certain amount of time.
    /// None if we never want to hear about that peer again.
    pub try_again_in: Option<u32>,
}
