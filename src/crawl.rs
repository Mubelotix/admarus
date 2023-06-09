use isahc::prelude::*;
use crate::prelude::*;

const RPC_URL: &str = "http://localhost:5001";
const MAX_HTML_LENGTH: usize = 15_000_000;

pub async fn list_pinned() -> Vec<String> {
    let mut rep = isahc::post_async(format!("{RPC_URL}/api/v0/pin/ls"), ()).await.unwrap();
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
    pinned
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

pub async fn explore_all(mut cids: Vec<String>) -> HashMap<String, Metadata> {
    let mut metadatas: HashMap<String, Metadata> = HashMap::new();
    while let Some(cid) = cids.pop() {
        let metadata = metadatas.entry(cid.clone()).or_default().to_owned();

        if let Some(new_links) = explore_dag(cid, metadata).await {
            for (cid, metadata) in new_links {
                cids.push(cid.clone());
                metadatas.entry(cid).or_default().merge(metadata);
            }
        }
    }

    metadatas
}

pub async fn explore_dag(cid: String, metadata: Metadata) -> Option<Vec<(String, Metadata)>> {
    let mut rep = isahc::post_async(format!("{RPC_URL}/api/v0/dag/get?arg={cid}"), ()).await.unwrap();
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

pub async fn collect_documents(links: HashMap<String, Metadata>) -> Vec<(String, Document, Metadata)> {
    let mut links = links.into_iter().collect::<Vec<_>>();
    links.sort_by_key(|(_,metadata)| !metadata.pathes.iter().any(|p| p.last().map(|p| p.ends_with(".html")).unwrap_or(false)));

    let mut documents = Vec::new();
    for (cid, metadata) in links {
        if let Some(document) = fetch_document(&cid, &metadata).await {
            documents.push((cid, document, metadata));
        }
    }

    documents
}

pub async fn fetch_document(cid: &String, metadata: &Metadata) -> Option<Document> {
    let mut rep = isahc::post_async(format!("{RPC_URL}/api/v0/cat?arg={cid}&length={MAX_HTML_LENGTH}"), ()).await.unwrap();
    let rep: Vec<u8> = rep.bytes().await.unwrap();

    if rep.starts_with(b"<!DOCTYPE html>") || rep.starts_with(b"<!doctype html>") {
        return Some(Document::Html(HtmlDocument::init(String::from_utf8_lossy(&rep).to_string())));
    }

    None
}
