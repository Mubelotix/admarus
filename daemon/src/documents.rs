use scraper::{Selector, Html, ElementRef};
use crate::prelude::*;

pub struct DocumentInspectionReport {
    pub text_content: String,
    pub description: Option<String>,
    // TODO: add structured data to documentinspectionreport
}

pub fn inspect_document(raw: Vec<u8>) -> Option<DocumentInspectionReport> {
    let raw_str = String::from_utf8_lossy(&raw);

    inspect_document_html(&raw_str)
}

pub fn generate_result(raw: Vec<u8>, cid: String, query: &Query, paths: Vec<Vec<String>>) -> Option<DocumentResult> {
    let raw_str = String::from_utf8_lossy(&raw);

    let mut result = generate_result_html(&raw_str, query)?;
    result.cid = cid;
    result.paths = paths;

    Some(result)
}

fn inspect_document_html(raw: &str) -> Option<DocumentInspectionReport> {
    if !raw.starts_with("<!DOCTYPE html>") && !raw.starts_with("<!doctype html>") {
        return None;
    }
    
    let document = Html::parse_document(raw);
    let mut filters = HashMap::new();

    // Get words
    let body_selector = Selector::parse("body").expect("Invalid body selector");
    let body_el = document.select(&body_selector).next();

    fn list_words(el: ElementRef, text_content: &mut String) {
        if ["script", "style"].contains(&el.value().name()) {
            return;
        }
        for child in el.children() {
            match child.value() {
                scraper::node::Node::Element(_) => {
                    let child_ref = ElementRef::wrap(child).expect("Child isn't an element");
                    list_words(child_ref, text_content)
                },
                scraper::node::Node::Text(text) => {
                    text_content.push(' ');
                    text_content.push_str(text.to_string().trim());
                },
                _ => (),
            }
        }

    }

    let mut text_content = String::new();
    if let Some(body_el) = body_el {
        list_words(body_el, &mut text_content);
    }

    // Retrieve description
    let description_selector = Selector::parse("meta[name=description]").expect("Invalid description selector");
    let description_el = document.select(&description_selector).next();
    let description = description_el.and_then(|el| el.value().attr("content").map(|c| c.to_string()));    

    // Get lang
    let html_selector = Selector::parse("html").expect("Invalid html selector");
    let html_el = document.select(&html_selector).next();
    let lang = html_el
        .and_then(|el| el.value().attr("lang").map(|lang| lang.trim()))
        .and_then(|l| l.split('-').next())
        .map(|l| l.to_string())
        .unwrap_or(String::from("unknown"));
    filters.insert("lang", lang);

    Some(DocumentInspectionReport { text_content, description })
}

#[allow(clippy::question_mark)]
fn generate_result_html(raw: &str, query: &Query) -> Option<DocumentResult> {
    let document = Html::parse_document(raw);
    let body_selector = Selector::parse("body").expect("Invalid body selector");
    let body_el = document.select(&body_selector).next();

    // Get lang
    let html_selector = Selector::parse("html").expect("Invalid html selector");
    let html_el = document.select(&html_selector).next();
    let lang = html_el
        .and_then(|el| el.value().attr("lang").map(|lang| lang.trim()))
        .and_then(|l| l.split('-').next())
        .map(|l| l.to_string())
        .unwrap_or(String::from("unknown"));

    // Retrieve title
    let title_selector = Selector::parse("title").expect("Invalid title selector");
    let title_el = document.select(&title_selector).next();
    let mut title = title_el.map(|el| el.text().collect::<Vec<_>>().join(" "));
    if title.as_ref().map(|t| t.trim().is_empty()).unwrap_or(false) {
        title = None;
    }

    // Retrieve h1
    let mut h1 = None;
    if title.is_none() {
        let h1_selector = Selector::parse("h1").expect("Invalid h1 selector");
        let h1_el = document.select(&h1_selector).next();
        h1 = h1_el.map(|el| el.text().collect::<Vec<_>>().join(" "));
        if h1.as_ref().map(|t| t.trim().is_empty()).unwrap_or(false) {
            h1 = None;
        }
    }
    
    if title.is_none() && h1.is_none() {
        return None;
    }

    // Retrieve favicons
    let favicon_selector = Selector::parse(r#"html>head>link[rel="icon"], html>head>link[rel="shortcut icon"], html>head>link[rel="apple-touch-icon"], html>head>link[rel="apple-touch-icon-precomposed"]"#).expect("Invalid icon selector");
    let mut favicons = Vec::new();
    for favicon_el in document.select(&favicon_selector) {
        let Some(href) = favicon_el.value().attr("href").map(|href| href.to_string()) else {continue};
        let Some(mime_type) = favicon_el.value().attr("type").map(|mime_type| mime_type.to_string()) else {continue};
        let Some(sizes) = favicon_el.value().attr("sizes").map(|sizes| sizes.to_string()) else {continue};
        favicons.push(FaviconDescriptor { href, mime_type, sizes });
    }

    // Retrieve description
    let description_selector = Selector::parse("meta[name=description]").expect("Invalid description selector");
    let description_el = document.select(&description_selector).next();
    let description = description_el.and_then(|el| el.value().attr("content").map(|c| c.to_string()));

    // Retrieve the most relevant extract
    fn extract_score(extract: &str, query_positive_terms: &[&String]) -> usize {
        let mut score = 0;
        let mut extract_words = extract.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 3).map(|w| w.to_lowercase()).collect::<Vec<_>>();
        if extract_words.is_empty() {
            return 0;
        }
        let first_word = extract_words.remove(0);
        if query_positive_terms.contains(&&first_word) {
            score += 4;
        }
        for query_positive_term in query_positive_terms {
            if extract_words.contains(query_positive_term) {
                score += 1;
            }
        }
        score
    }
    let body = document.select(&Selector::parse("body").expect("Invalid body selector")).next()?;
    let query_positive_terms = query.positive_terms();
    let fragments = body.text().collect::<Vec<_>>();
    let mut best_extract = "";
    let mut best_extract_score = 0;
    for fragment in fragments {
        if fragment.len() >= 350 || fragment.len() <= 50 {
            continue;
        }
        let score = extract_score(fragment, &query_positive_terms);
        if score > best_extract_score {
            best_extract_score = score;
            best_extract = fragment;
        }
    }
    let extract = match best_extract_score > 0 {
        true => Some(best_extract.to_string()),
        false => None,
    };
    
    if description.is_none() && extract.is_none() {
        return None;
    }

    // Retrieve images and videos
    /*fn list_media(el: ElementRef, media: &mut Vec<StructuredData>) {
        'init: {match el.value().name() {
            "img" => {
                let Some(src) = el.value().attr("src").map(|s| s.to_string()) else {break 'init};
                let mut new_media = schemas::types::ImageObject::new();
                new_media.media_object.set_content_url(SchemaUrl::from(src));
                if let Some(alt) = el.value().attr("alt") {
                    new_media.media_object.creative_work.thing.set_description(SchemaText::from(alt));
                }
                media.push(StructuredData::ImageObject(new_media));
            },
            "video" | "picture" => {
                // TODO
            },
            _ => (),
        }}
        for child in el.children() {
            if let scraper::node::Node::Element(_) = child.value() {
                let child_ref = ElementRef::wrap(child).expect("Child isn't an element");
                list_media(child_ref, media)
            }
        }
    }
    let mut media = Vec::new();
    if let Some(body_el) = body_el {
        list_media(body_el, &mut media);
    }*/

    // Count words
    #[allow(clippy::too_many_arguments)]
    fn count_words(
        el: ElementRef, query_positive_terms: &[&String], term_counts: &mut Vec<WordCount>, word_count: &mut WordCount, common_words: Option<&[&str]>,
        common_words_bytes: &mut usize, uncommon_words_bytes: &mut usize,
        mut h1: bool, mut h2: bool, mut h3: bool, mut h4: bool, mut h5: bool, mut h6: bool, mut strong: bool, mut em: bool, mut small: bool, mut s: bool
    ) {
        match el.value().name() {
            "h1" => h1 = true,
            "h2" => h2 = true,
            "h3" => h3 = true,
            "h4" => h4 = true,
            "h5" => h5 = true,
            "h6" => h6 = true,
            "strong" => strong = true,
            "em" => em = true,
            "small" => small = true,
            "s" => s = true,
            "script" | "style" => return,
            _ => (),
        }
        for child in el.children() {
            match child.value() {
                scraper::node::Node::Element(_) => {
                    let child_ref = ElementRef::wrap(child).expect("Child isn't an element");
                    count_words(child_ref, query_positive_terms, term_counts, word_count, common_words, common_words_bytes, uncommon_words_bytes, h1, h2, h3, h4, h5, h6, strong, em, small, s)
                },
                scraper::node::Node::Text(text) => {
                    let text = text.to_lowercase();
                    let words = text
                        .split(|c: char| !c.is_ascii_alphanumeric())
                        .filter(|w| w.len() >= 3)
                        .map(|w| w.to_string());
                    for word in words {
                        if let Some(common_words) = common_words {
                            if common_words.sorted_contains(&word) {
                                *common_words_bytes += word.len();
                            } else {
                                *uncommon_words_bytes += word.len();
                            }
                        }
                        if let Some(i) = query_positive_terms.iter().position(|q| *q == &word) {
                            let term_count = term_counts.get_mut(i).expect("term_counts not initialized properly");
                            term_count.add(h1, h2, h3, h4, h5, h6, strong, em, small, s)
                        }
                        word_count.add(h1, h2, h3, h4, h5, h6, strong, em, small, s);
                    }
                },
                _ => (),
            }
        }
    }
    let common_words = match lang.as_str() {
        "en" => Some(word_lists::WORDS_EN),
        _ => None,
    };
    let (mut common_words_bytes, mut uncommon_words_bytes) = (0, 0);
    let mut term_counts = query_positive_terms.iter().map(|_| WordCount::default()).collect::<Vec<_>>();
    let mut word_count = WordCount::default();
    count_words(
        body, &query_positive_terms, &mut term_counts, &mut word_count, common_words,
        &mut common_words_bytes, &mut uncommon_words_bytes,
        false, false, false, false, false, false, false, false, false, false
    );
    let common_words = common_words.map(|_| common_words_bytes as f64 / (common_words_bytes + uncommon_words_bytes) as f64);

    Some(DocumentResult {
        cid: String::new(),
        paths: Vec::new(),
        favicons,
        title,
        h1,
        description,
        extract,

        structured_data: Vec::new(),

        term_counts,
        word_count,
        common_words,
    })
}
