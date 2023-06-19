use scraper::Selector;

use crate::prelude::*;

#[derive(Serialize, Deserialize)]
pub enum Document {
    Html(HtmlDocument),
}

impl Document {
    pub fn words(&self) -> impl Iterator<Item = &String> {
        match self {
            Document::Html(html) => html.words(),
        }
    }

    pub fn into_result(self, cid: String, metadata: Metadata) -> DocumentResult {
        match self {
            Document::Html(html) => html.into_result(cid, metadata),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct HtmlDocument {
    raw: String,
    words: Vec<String>,
}

impl HtmlDocument {
    pub fn init(raw: String) -> HtmlDocument {
        use scraper::Html;

        let document = Html::parse_document(&raw);
        let selector = Selector::parse("body").unwrap();
        let root = document.select(&selector).next().unwrap();
        let words = root.text().collect::<Vec<_>>().join(" ").to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 3).map(|w| w.to_string()).collect();

        HtmlDocument {
            raw,
            words
        }
    }

    pub fn words(&self) -> impl Iterator<Item = &String> {
        self.words.iter()
    }

    pub fn into_result(self, cid: String, metadata: Metadata) -> DocumentResult {
        use scraper::Html;

        let document = Html::parse_document(&self.raw);

        let title_selector = Selector::parse("title").unwrap();
        let title_el = document.select(&title_selector).next();
        let title = title_el.map(|el| el.text().collect::<Vec<_>>().join(" "));

        let description_selector = Selector::parse("meta[name=description]").unwrap();
        let description_el = document.select(&description_selector).next();
        let description = description_el.map(|el| el.value().attr("content").unwrap().to_string());

        DocumentResult {
            cid,
            icon_cid: None,
            domain: None,
            title: title.unwrap_or_default(),
            description: description.unwrap_or_default(),
        }
    }
}


