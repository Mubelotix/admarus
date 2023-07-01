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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
    pub paths: Vec<Vec<String>>,
    pub size: Option<u64>,
}

impl Metadata {
    fn merge(&mut self, other: Metadata) {
        self.paths.extend(other.paths);
        self.size = self.size.or(other.size);
    }
}

pub async fn explore_all(ipfs_rpc: &str, mut cids: Vec<String>) -> HashMap<String, Metadata> {
    let mut metadatas: HashMap<String, Metadata> = HashMap::new();
    while let Some(cid) = cids.pop() {
        let metadata = metadatas.get(&cid);

        match explore_dag(ipfs_rpc, cid, metadata).await {
            Ok(Some(new_links)) => for (cid, metadata) in new_links {
                cids.push(cid.clone());
                metadatas.entry(cid).or_default().merge(metadata);
            }
            Ok(None) => (),
            Err(e) => warn!("Error while exploring dag: {e:?}"),
        }
    }

    metadatas
}

pub async fn get_dag(ipfs_rpc: &str, cid: &str) -> Result<serde_json::Value, IpfsRpcError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/dag/get?arg={cid}")).send().await?;
    let rep = rep.text().await?;
    let rep = serde_json::from_str::<serde_json::Value>(&rep)?;
    Ok(rep)
}

pub async fn explore_dag(ipfs_rpc: &str, cid: String, metadata: Option<&Metadata>) -> Result<Option<Vec<(String, Metadata)>>, IpfsRpcError> {
    let dag = get_dag(ipfs_rpc, &cid).await?;
    
    let data = dag
        .get("Data").unwrap_or(&dag)
        .get("/").ok_or(InvalidResponse("/ expected on Data"))?
        .get("bytes").ok_or(InvalidResponse("bytes expected on /"))?
        .as_str().ok_or(InvalidResponse("bytes expected to be a string"))?;

    if data != "CAE" { // This is the legacy IPFS UnixFS format according to ipfs-search.com guys. I personnaly think it indicates the ADL used.
        return Ok(None);
    }

    let links_json = dag
        .get("Links")
        .and_then(|l| l.as_array())
        .map(|l| l.to_owned())
        .unwrap_or_default();

    let is_dns_pins = dag
        .get("DNS-Pins")
        .and_then(|l| l.as_bool())
        .unwrap_or(false);

    let mut links = Vec::new();
    for new_link in links_json {
        let child_cid = new_link
            .get("Hash").ok_or(InvalidResponse("Hash expected on link"))?
            .get("/").ok_or(InvalidResponse("/ expected on Hash"))?
            .as_str().ok_or(InvalidResponse("Hash expected to be a string"))?;
        let name = new_link
            .get("Name").ok_or(InvalidResponse("Name expected on link"))?
            .as_str().ok_or(InvalidResponse("Name expected to be a string"))?;
        let size = new_link
            .get("Tsize").and_then(|l| l.as_u64());
        let mut paths = metadata.map(|m| m.paths.clone()).unwrap_or_default();
        paths.iter_mut().for_each(|p| p.push(name.to_owned()));
        match is_dns_pins && metadata.is_none() {
            true => {
                let domain = name.split_whitespace().next().unwrap_or(name);
                paths.push(vec![domain.to_owned()]);
            },
            false => paths.push(vec![cid.to_owned(), name.to_owned()]),
        }
        paths.push(vec![String::from(child_cid)]);

        links.push((child_cid.to_owned(), Metadata {
            paths,
            size,
        }));
    }

    Ok(Some(links))
}

pub async fn collect_documents(ipfs_rpc: &str, links: HashMap<String, Metadata>) -> Vec<(String, Document, Metadata)> {
    let mut links = links.into_iter().collect::<Vec<_>>();
    links.sort_by_key(|(_,metadata)| !metadata.paths.iter().any(|p| p.last().map(|p| p.ends_with(".html")).unwrap_or(false)));

    let mut documents = Vec::new();
    for (cid, metadata) in links {
        match fetch_document(ipfs_rpc, &cid).await {
            Ok(Some(document)) => documents.push((cid, document, metadata)),
            Ok(None) => (),
            Err(e) => warn!("Error while fetching document: {e:?}"),
        }
    }

    documents
}

pub async fn fetch_document(ipfs_rpc: &str, cid: &String) -> Result<Option<Document>, IpfsRpcError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/cat?arg={cid}&length={MAX_HTML_LENGTH}")).send().await?;
    let rep: Vec<u8> = rep.bytes().await?.to_vec();

    if rep.starts_with(b"<!DOCTYPE html>") || rep.starts_with(b"<!doctype html>") {
        return Ok(Some(Document::Html(HtmlDocument::init(String::from_utf8_lossy(&rep).to_string()))));
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
    let rep = client.post(format!("{ipfs_rpc}/api/v0/dag/put?pin={pin}"))
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
