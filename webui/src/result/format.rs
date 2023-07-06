use crate::prelude::*;
use yew::virtual_dom::{VList, VText, VTag};

impl DocumentResult {
    pub fn rank_paths(&mut self) {
        // TODO: sort using more advanced algorithm
        self.paths.sort_by(|a, b| b.first().map(|f| f.contains('.')).cmp(&a.first().map(|f| f.contains('.'))).then_with(|| b.len().cmp(&a.len())));
    }
    
    pub fn format_result_title(&self) -> String {
        match self.title {
            Some(ref title) => title.clone(),
            None => match self.h1 {
                Some(ref h1) => h1.clone(),
                None => match self.paths.first() {
                    Some(path) => path.last().unwrap_or(&self.cid).clone(),
                    None => self.cid.clone(),
                }
            }
        }
    }

    pub fn format_best_addr(&self) -> String {
        let mut best_addr = match self.paths.first() {
            Some(f) => f.as_slice(),
            None => return format!("ipfs://{}", self.cid),
        };

        if best_addr.last().map(|l| l == "index.html").unwrap_or(false) {
            best_addr = &best_addr[..best_addr.len() - 1];
        }

        match best_addr.first().map(|f| f.contains('.')).unwrap_or(false) {
            true => format!("ipns://{}", best_addr.join("/")),
            false => format!("ipfs://{}", best_addr.join("/")),
        }
    }

    pub fn view_desc(&self, query: &Query) -> VList {
        // TODO: this is a copy of daemon code
        fn extract_score(extract: &str, query: &[&String]) -> usize {
            let mut score = 0;
            let mut extract_words = extract.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 3).map(|w| w.to_lowercase()).collect::<Vec<_>>();
            if extract_words.is_empty() {
                return 0;
            }
            let first_word = extract_words.remove(0);
            if query.contains(&&first_word) {
                score += 4;
            }
            for word in query {
                if extract_words.contains(word) {
                    score += 1;
                }
            }
            score
        }

        let query_terms = query.positive_terms();
        let desc = match (&self.description, &self.extract) {
            (Some(desc), Some(extract)) => {
                if extract_score(desc, query_terms.as_slice()) >= extract_score(extract, query_terms.as_slice()) {
                    desc
                } else {
                    extract
                }
            }
            (Some(desc), None) => desc,
            (None, Some(extract)) => extract,
            (None, None) => return VList::new(),
        };
        let mut i = 0;
        let mut added = 0;
        let mut vlist = VList::new();
        for part in desc.split_inclusive(|c: char| !c.is_ascii_alphanumeric()) {
            let part_len = part.len();
            let word = part.trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
            if word.len() >= 3 && query_terms.contains(&&word.to_lowercase()) {
                if i - added > 0 {
                    let unbolded_text = desc[added..i].to_string();
                    vlist.add_child(VText::new(unbolded_text).into());
                }
                let bolded_text = desc[i..i + word.len()].to_string();
                let mut b_el = VTag::new("b");
                b_el.add_child(VText::new(bolded_text).into());
                vlist.add_child(b_el.into());
                added = i + word.len();
            }
            i += part_len;
        }
        if i - added > 0 {
            let unbolded_text = desc[added..i].to_string();
            vlist.add_child(VText::new(unbolded_text).into());
        }

        vlist
    }
}
