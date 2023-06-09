use reqwest::Client;
use crate::prelude::*;

const MAX_HTML_LENGTH: usize = 15_000_000;

#[derive(Debug)]
pub enum CrawlingError {
    ReqwestError(reqwest::Error),
}

impl From<reqwest::Error> for CrawlingError {
    fn from(e: reqwest::Error) -> Self {
        CrawlingError::ReqwestError(e)
    }
}

pub async fn list_pinned(ipfs_rpc: &str) -> Result<Vec<String>, CrawlingError> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/pin/ls")).send().await?;
    let rep = rep.text().await.unwrap();
    let data = serde_json::from_str::<serde_json::Value>(&rep).unwrap();
    let keys = data.get("Keys").unwrap().as_object().unwrap();
    let mut pinned = Vec::new();
    for (key, value) in keys.into_iter() {
        let ty = value.get("Type").unwrap().as_str().unwrap();
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

        if let Some(new_links) = explore_dag(ipfs_rpc, cid, metadata).await {
            for (cid, metadata) in new_links {
                cids.push(cid.clone());
                metadatas.entry(cid).or_default().merge(metadata);
            }
        }
    }

    metadatas
}

pub async fn explore_dag(ipfs_rpc: &str, cid: String, metadata: Metadata) -> Option<Vec<(String, Metadata)>> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/dag/get?arg={cid}")).send().await.unwrap();
    let rep = rep.text().await.unwrap();
    let rep = serde_json::from_str::<serde_json::Value>(&rep).unwrap();
    
    let data = rep.get("Data").unwrap_or(&rep).get("/").unwrap().get("bytes").unwrap().as_str().unwrap();
    if data != "CAE" {
        return None;
    }

    let links_json = rep.get("Links").map(|l| l.as_array().unwrap().to_owned()).unwrap_or_default();
    let mut links = Vec::new();
    for new_link in links_json {
        let hash = new_link.get("Hash").unwrap().get("/").unwrap().as_str().unwrap();
        let name = new_link.get("Name").unwrap().as_str().unwrap();
        let size = new_link.get("Tsize").unwrap().as_u64().unwrap();
        let mut pathes = metadata.pathes.clone();
        pathes.iter_mut().for_each(|p| p.push(name.to_owned()));

        links.push((hash.to_owned(), Metadata {
            pathes,
            size: Some(size),
        }));
    }

    Some(links)
}

pub async fn collect_documents(ipfs_rpc: &str, links: HashMap<String, Metadata>) -> Vec<(String, Document, Metadata)> {
    let mut links = links.into_iter().collect::<Vec<_>>();
    links.sort_by_key(|(_,metadata)| !metadata.pathes.iter().any(|p| p.last().map(|p| p.ends_with(".html")).unwrap_or(false)));

    let mut documents = Vec::new();
    for (cid, metadata) in links {
        if let Some(document) = fetch_document(ipfs_rpc, &cid).await {
            documents.push((cid, document, metadata));
        }
    }

    documents
}

pub async fn fetch_document(ipfs_rpc: &str, cid: &String) -> Option<Document> {
    let client = Client::new();
    let rep = client.post(format!("{ipfs_rpc}/api/v0/cat?arg={cid}&length={MAX_HTML_LENGTH}")).send().await.unwrap();
    let rep: Vec<u8> = rep.bytes().await.unwrap().to_vec();

    if rep.starts_with(b"<!DOCTYPE html>") || rep.starts_with(b"<!doctype html>") {
        return Some(Document::Html(HtmlDocument::init(String::from_utf8_lossy(&rep).to_string())));
    }

    None
}
