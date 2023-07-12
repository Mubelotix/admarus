use crate::prelude::*;

#[derive(Debug)]
pub enum CensusRpcError {
    Reqwest(reqwest::Error),
    Json(serde_json::Error),
    Signing(libp2p::identity::SigningError),
    Status(u16, String),
}

impl From<reqwest::Error> for CensusRpcError {
    fn from(e: reqwest::Error) -> Self {
        CensusRpcError::Reqwest(e)
    }
}

impl From<libp2p::identity::SigningError> for CensusRpcError {
    fn from(e: libp2p::identity::SigningError) -> Self {
        CensusRpcError::Signing(e)
    }
}

impl From<serde_json::Error> for CensusRpcError {
    fn from(e: serde_json::Error) -> Self {
        CensusRpcError::Json(e)
    }
}

#[derive(Serialize, Deserialize)]
struct ApiRecord {
    record: Record,
    public_key: Vec<u8>,
    signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hashable)]
pub struct Record {
    pub peer_id: String,
    pub addrs: Vec<String>,
    pub folders: Vec<(String, u64)>,
}

pub async fn submit_census_record(census_rpc: &str, record: Record, keys: Keypair) -> Result<(), CensusRpcError> {
    let hash = record.hash();
    let signature = keys.sign(hash.as_slice())?;
    let api_record = ApiRecord {
        record,
        public_key: keys.public().encode_protobuf(),
        signature,
    };

    let client = Client::new();
    let resp = client.post(format!("{census_rpc}/api/v0/submit"))
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&api_record)?)
        .send()
        .await?;

    if resp.status() == 200 {
        Ok(())
    } else {
        let status = resp.status().as_u16();
        let text = resp.text().await?;
        Err(CensusRpcError::Status(status, text))
    }
}

pub async fn get_census_peers(census_rpc: &str, exclude: Vec<PeerId>) -> Result<Vec<(PeerId, Vec<Multiaddr>)>, CensusRpcError> {
    let mut url = format!("{census_rpc}/api/v0/peers");
    if !exclude.is_empty() {
        url.push_str("?exclude=");
        url.push_str(&exclude.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(","));
    }

    let client = Client::new();
    let resp = client.get(url)
        .send()
        .await?;
    let status = resp.status().as_u16();
    let body = resp.text().await?;
    if status != 200 {
        return Err(CensusRpcError::Status(status, body));
    }

    let value = serde_json::from_str::<Vec<(String, Vec<String>)>>(&body)?;
    let mut peers = Vec::new();
    for (peer_id, addrs) in value {
        let Ok(peer_id) = peer_id.parse() else {
            warn!("Invalid peer ID in census response: {peer_id}");
            continue
        };
        let addrs = addrs.into_iter().filter_map(|addr| addr.parse().ok()).collect();
        peers.push((peer_id, addrs));
    }

    Ok(peers)
}
