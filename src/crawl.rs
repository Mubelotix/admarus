use reqwest::Client;
use crate::prelude::*;

const MAX_HTML_LENGTH: usize = 15_000_000;

#[derive(Debug)]
pub enum FetchingError {
    ReqwestError(reqwest::Error),
    InvalidJson(serde_json::Error),
    InvalidResponse(&'static str),
}

impl From<reqwest::Error> for FetchingError {
    fn from(e: reqwest::Error) -> Self {
        FetchingError::ReqwestError(e)
    }
}

impl From<serde_json::Error> for FetchingError {
    fn from(e: serde_json::Error) -> Self {
        FetchingError::InvalidJson(e)
    }
}

use FetchingError::InvalidResponse;

pub async fn list_pinned(ipfs_rpc: &str) -> Result<Vec<String>, FetchingError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/pin/ls")).send().await?;
    let rep = rep.text().await?;
    let data = serde_json::from_str::<serde_json::Value>(&rep)?;
    let keys = data
        .get("Keys").ok_or(InvalidResponse("Keys expected on data"))?
        .as_object().ok_or(InvalidResponse("Keys expected to be an object"))?;

    let mut pinned = Vec::new();
    for (key, value) in keys.into_iter() {
        let ty = value
            .get("Type").ok_or(InvalidResponse("Type expected on value"))?
            .as_str().ok_or(InvalidResponse("Type expected to be a string"))?;
        if ty != "indirect" {
            pinned.push(key.clone());
        }
    }

    Ok(pinned)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
    pub pathes: Vec<Vec<String>>,
    pub size: Option<u64>,
}

impl Metadata {
    fn merge(&mut self, other: Metadata) {
        self.pathes.extend(other.pathes);
        self.size = self.size.or(other.size);
    }
}

pub async fn explore_all(ipfs_rpc: &str, mut cids: Vec<String>) -> HashMap<String, Metadata> {
    let mut metadatas: HashMap<String, Metadata> = HashMap::new();
    while let Some(cid) = cids.pop() {
        let metadata = metadatas.entry(cid.clone()).or_default().to_owned();

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

pub async fn explore_dag(ipfs_rpc: &str, cid: String, metadata: Metadata) -> Result<Option<Vec<(String, Metadata)>>, FetchingError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/dag/get?arg={cid}")).send().await?;
    let rep = rep.text().await?;
    let rep = serde_json::from_str::<serde_json::Value>(&rep)?;
    
    let data = rep
        .get("Data").unwrap_or(&rep)
        .get("/").ok_or(InvalidResponse("/ expected on Data"))?
        .get("bytes").ok_or(InvalidResponse("bytes expected on /"))?
        .as_str().ok_or(InvalidResponse("bytes expected to be a string"))?;

    if data != "CAE" {
        return Ok(None);
    }

    let links_json = rep
        .get("Links")
        .and_then(|l| l.as_array())
        .map(|l| l.to_owned())
        .unwrap_or_default();

    let mut links = Vec::new();
    for new_link in links_json {
        let hash = new_link
            .get("Hash").ok_or(InvalidResponse("Hash expected on link"))?
            .get("/").ok_or(InvalidResponse("/ expected on Hash"))?
            .as_str().ok_or(InvalidResponse("Hash expected to be a string"))?;
        let name = new_link
            .get("Name").ok_or(InvalidResponse("Name expected on link"))?
            .as_str().ok_or(InvalidResponse("Name expected to be a string"))?;
        let size = new_link
            .get("Tsize").ok_or(InvalidResponse("Tsize expected on link"))?
            .as_u64().ok_or(InvalidResponse("Tsize expected to be a u64"))?;
        let mut pathes = metadata.pathes.clone();
        pathes.iter_mut().for_each(|p| p.push(name.to_owned()));

        links.push((hash.to_owned(), Metadata {
            pathes,
            size: Some(size),
        }));
    }

    Ok(Some(links))
}

pub async fn collect_documents(ipfs_rpc: &str, links: HashMap<String, Metadata>) -> Vec<(String, Document, Metadata)> {
    let mut links = links.into_iter().collect::<Vec<_>>();
    links.sort_by_key(|(_,metadata)| !metadata.pathes.iter().any(|p| p.last().map(|p| p.ends_with(".html")).unwrap_or(false)));

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

pub async fn fetch_document(ipfs_rpc: &str, cid: &String) -> Result<Option<Document>, FetchingError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/cat?arg={cid}&length={MAX_HTML_LENGTH}")).send().await?;
    let rep: Vec<u8> = rep.bytes().await?.to_vec();

    if rep.starts_with(b"<!DOCTYPE html>") || rep.starts_with(b"<!doctype html>") {
        return Ok(Some(Document::Html(HtmlDocument::init(String::from_utf8_lossy(&rep).to_string()))));
    }

    Ok(None)
}
