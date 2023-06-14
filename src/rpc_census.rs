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
}

async fn publish_record(census_rpc: &str, record: Record, keys: Keypair) -> Result<(), CensusRpcError> {
    let hash = record.hash();
    let signature = keys.sign(&hash)?;
    let api_record = ApiRecord {
        record,
        public_key: keys.public().encode_protobuf(),
        signature,
    };

    let client = Client::new();
    let resp = client.post(format!("{census_rpc}/api/v0/publish"))
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
