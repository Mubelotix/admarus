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
}


