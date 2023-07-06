use crate::prelude::*;

const MAX_HTML_LENGTH: usize = 15_000_000;

#[derive(Debug)]
pub enum IpfsRpcError {
    Reqwest(reqwest::Error),
    Json(serde_json::Error),
    InvalidResponse(&'static str),
}

impl From<reqwest::Error> for IpfsRpcError {
    fn from(e: reqwest::Error) -> Self {
        IpfsRpcError::Reqwest(e)
    }
}

impl From<serde_json::Error> for IpfsRpcError {
    fn from(e: serde_json::Error) -> Self {
        IpfsRpcError::Json(e)
    }
}

impl std::fmt::Display for IpfsRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpfsRpcError::Reqwest(e) => write!(f, "ReqwestError: {e}"),
            IpfsRpcError::Json(e) => write!(f, "InvalidJson: {e}"),
            IpfsRpcError::InvalidResponse(e) => write!(f, "InvalidResponse: {e}"),
        }
    }
}

use IpfsRpcError::InvalidResponse;
use reqwest::StatusCode;

pub async fn list_pinned(ipfs_rpc: &str) -> Result<Vec<String>, IpfsRpcError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/pin/ls?type=recursive")).send().await?;
    let rep = rep.text().await?;
    let data = serde_json::from_str::<serde_json::Value>(&rep)?;
    let keys = data
        .get("Keys").ok_or(InvalidResponse("Keys expected on data"))?
        .as_object().ok_or(InvalidResponse("Keys expected to be an object"))?;
    Ok(keys.into_iter().map(|(k,_)| k).cloned().collect())
}

pub async fn get_dag(ipfs_rpc: &str, cid: &str) -> Result<serde_json::Value, IpfsRpcError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/dag/get?arg={cid}")).send().await?;
    let rep = rep.text().await?;
    let rep = serde_json::from_str::<serde_json::Value>(&rep)?;
    Ok(rep)
}

pub async fn ls(ipfs_rpc: &str, parent_cid: String) -> Result<Vec<(String, String, bool)>, IpfsRpcError> {
    // TODO: streaming

    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/ls?arg={parent_cid}")).send().await?;
    let rep = rep.text().await?;
    let rep = serde_json::from_str::<serde_json::Value>(&rep)?;

    let objects = rep
        .get("Objects").ok_or(InvalidResponse("Objects expected on data"))?
        .as_array().ok_or(InvalidResponse("Objects expected to be an array"))?;

    // TODO: dns pins

    let mut rep = Vec::new();
    for object in objects {
        let links = object
            .get("Links").ok_or(InvalidResponse("Links expected on object"))?
            .as_array().ok_or(InvalidResponse("Links expected to be an array"))?;

        for link in links {
            let child_cid = link
                .get("Hash").ok_or(InvalidResponse("Hash expected on link"))?
                .as_str().ok_or(InvalidResponse("Hash expected to be a string"))?;
            let name = link
                .get("Name").ok_or(InvalidResponse("Name expected on link"))?
                .as_str().ok_or(InvalidResponse("Name expected to be a string"))?;
            let ty = link
                .get("Type").ok_or(InvalidResponse("Type expected on link"))?
                .as_u64().ok_or(InvalidResponse("Type expected to be a number"))?;

            rep.push((child_cid.to_owned(), name.to_string(), ty == 2));
        }
    }

    Ok(rep)
}

pub async fn fetch_document(ipfs_rpc: &str, cid: &String) -> Result<Option<Document>, IpfsRpcError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/cat?arg={cid}&length={MAX_HTML_LENGTH}")).send().await?;
    let rep: Vec<u8> = rep.bytes().await?.to_vec();

    if rep.starts_with(b"<!DOCTYPE html>") || rep.starts_with(b"<!doctype html>") {
        return Ok(Some(Document::Html(HtmlDocument::init(cid.to_owned(), String::from_utf8_lossy(&rep).to_string()))));
    }

    Ok(None)
}

pub async fn get_ipfs_peers(ipfs_rpc: &str) -> Result<Vec<(PeerId, Multiaddr)>, IpfsRpcError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/swarm/peers")).send().await?;
    let rep = rep.text().await?;
    let rep = serde_json::from_str::<serde_json::Value>(&rep)?;
    let peers = rep
        .get("Peers").ok_or(InvalidResponse("Peers expected on data"))?
        .as_array().ok_or(InvalidResponse("Peers expected to be an array"))?;

    let mut results = Vec::new();
    for peer in peers {
        // Get addr
        let addr = peer
            .get("Addr").ok_or(InvalidResponse("Addr expected on peer"))?
            .as_str().ok_or(InvalidResponse("Addr expected to be a string"))?;
        let Ok(addr) = addr.parse() else {
            warn!("Invalid multiaddr: {addr}");
            continue;
        };

        // Get peer id
        let peer_id = peer
            .get("Peer").ok_or(InvalidResponse("Peer expected on peer"))?
            .as_str().ok_or(InvalidResponse("Peer expected to be a string"))?;
        let Ok(peer_id) = peer_id.parse() else {
            warn!("Invalid peer id: {peer_id}");
            continue;
        };
        
        results.push((peer_id, addr));
    }

    Ok(results)
}

pub async fn put_dag(ipfs_rpc: &str, dag_json: String, pin: bool) -> Result<String, IpfsRpcError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/dag/put?pin={pin}&store-codec=dag-pb"))
        .multipart(reqwest::multipart::Form::new().text("object data", dag_json))
        .send().await?;
    let rep = rep.text().await?;
    let rep = serde_json::from_str::<serde_json::Value>(&rep)?;
    let cid = rep
        .get("Cid").ok_or(InvalidResponse("Cid expected on data"))?
        .get("/").ok_or(InvalidResponse("/ expected on Cid"))?
        .as_str().ok_or(InvalidResponse("Cid expected to be a string"))?;
    Ok(cid.to_owned())
}

pub async fn add_pin(ipfs_rpc: &str, cid: &str) -> Result<(), IpfsRpcError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/pin/add?arg={cid}&recursive=true")).send().await?;
    match rep.status() {
        StatusCode::OK => Ok(()),
        _ => {
            let rep = rep.text().await?;
            error!("Failed to replace pin: {rep}");
            Err(InvalidResponse("Status code not OK"))
        },
    }
}

pub async fn remove_pin(ipfs_rpc: &str, cid: &str) -> Result<(), IpfsRpcError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/pin/rm?arg={cid}&recursive=true")).send().await?;
    match rep.status() {
        StatusCode::OK => Ok(()),
        _ => {
            let rep = rep.text().await?;
            error!("Failed to replace pin: {rep}");
            Err(InvalidResponse("Status code not OK"))
        },
    }
}
