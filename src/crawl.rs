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

#[derive(Debug, Serialize, Deserialize)]
pub struct Link {
    pub cid: String,
    pub path: Vec<String>,
    pub name: Option<String>,
    pub size: Option<u64>,
}

pub async fn explore_all(cids: Vec<String>) -> Vec<Link> {
    let mut links = Vec::new();
    for cid in cids {
        links.push(Link {
            cid: cid.clone(),
            path: vec![cid],
            name: None,
            size: None,
        })
    }

    let mut files = Vec::new();
    while let Some(link) = links.pop() {
        match explore_dag(&link).await {
            Some(mut new_links) => links.append(&mut new_links),
            None => files.push(link)
        }
    }

    files
}

pub async fn explore_dag(link: &Link) -> Option<Vec<Link>> {
    let mut rep = isahc::post_async(format!("{RPC_URL}/api/v0/dag/get?arg={}", link.cid), ()).await.unwrap();
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
        let mut path = link.path.clone();
        path.push(name.to_owned());

        links.push(Link {
            cid: hash.to_owned(),
            path,
            name: Some(name.to_owned()), // Todo none
            size: Some(size),
        });
    }

    Some(links)
}

pub async fn collect_documents(mut links: Vec<Link>) -> Vec<(Document, Link)> {
    links.sort_by_key(|l| !l.path.last().map(|p| p.ends_with(".html")).unwrap_or(false));

    let mut documents = Vec::new();
    for link in links {
        if let Some(document) = fetch_document(&link).await {
            documents.push((document, link));
        }
    }

    documents
}

pub async fn fetch_document(link: &Link) -> Option<Document> {
    let mut rep = isahc::post_async(format!("{RPC_URL}/api/v0/cat?arg={}&length={MAX_HTML_LENGTH}", link.cid), ()).await.unwrap();
    let rep: Vec<u8> = rep.bytes().await.unwrap();

    if rep.starts_with(b"<!DOCTYPE html>") || rep.starts_with(b"<!doctype html>") {
        return Some(Document::Html(HtmlDocument::init(String::from_utf8_lossy(&rep).to_string())));
    }

    None
}
