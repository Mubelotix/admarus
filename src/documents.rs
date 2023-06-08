use scraper::Selector;

use crate::prelude::*;

pub struct Document {
    pub link: Link,
    pub document: DocumentContent,
}

impl Document {
    pub fn html(link: Link, document: HtmlDocument) -> Document {
        Document {
            link,
            document: DocumentContent::Html(document),
        }
    }

    pub fn words(&self) -> impl Iterator<Item = &String> {
        match &self.document {
            DocumentContent::Html(document) => document.words(),
        }
    }
}

pub enum DocumentContent {
    Html(HtmlDocument),
}

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
}


