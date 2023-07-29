use super::*;

#[derive(Debug, Clone)]
pub struct PeerListQuery {
    pub peer_id: PeerId,
    pub protocol_version: Option<String>,
    pub agent_version: Option<String>,
    pub protocols: Option<Vec<String>>,
    pub metadata: Option<Vec<u8>>,
    pub max_results: Option<usize>,
}

impl PeerListQuery {
    pub fn new(peer_id: PeerId) -> PeerListQuery {
        PeerListQuery {
            peer_id,
            protocol_version: None,
            agent_version: None,
            protocols: None,
            metadata: None,
            max_results: None,
        }
    }

    pub fn with_protocol_version(mut self, protocol_version: String) -> Self {
        self.protocol_version = Some(protocol_version);
        self
    }

    pub fn with_agent_version(mut self, agent_version: String) -> Self {
        self.agent_version = Some(agent_version);
        self
    }

    pub fn with_protocols(mut self, protocols: Vec<String>) -> Self {
        self.protocols = Some(protocols);
        self
    }

    pub fn with_protocol(mut self, protocol: String) -> Self {
        self.protocols = match self.protocols {
            Some(mut protocols) => {
                protocols.push(protocol);
                Some(protocols)
            },
            None => Some(vec![protocol]),
        };
        self
    }

    pub fn with_metadata(mut self, metadata: Vec<u8>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}
