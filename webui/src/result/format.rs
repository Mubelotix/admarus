use crate::prelude::*;
use yew::virtual_dom::{VList, VText, VTag};

fn format_path_for_gateway(mut path: &[String]) -> Option<String> {
    if path.last().map(|l| l == "index.html").unwrap_or(false) {
        path = &path[..path.len() - 1];
    }

    match path.first().map(|f| f.contains('.')).unwrap_or(false) {
        true => {
            let mut domain = path[0].to_owned();
            domain = domain.replace('-', "--");
            domain = domain.replace('.', "-");
            path = &path[1..];
            Some(format!("https://{domain}.ipns.dweb.link/{}", path.join("/")))
        },
        false if !path.is_empty() => {
            let first = &path[0];
            path = &path[1..];
            Some(format!("https://{first}.ipfs.dweb.link/{}", path.join("/")))
        }
        false => None,
    }

}

impl FaviconDescriptor {
    pub fn format_srcset(&self, doc_path: &[String]) -> Option<String> {
        if self.href.starts_with("http://") || self.href.starts_with("https://") || self.href.starts_with("ipfs://") || self.href.starts_with("ipns://") {
            return Some(self.href.to_owned());
        }
        
        let tmp = self.process_relative_srcset(doc_path).and_then(|path| format_path_for_gateway(&path));
        log!("{doc_path:?} -> {} -> {tmp:?}", self.href);
        tmp
    }

    fn process_relative_srcset(&self, mut doc_path: &[String]) -> Option<Vec<String>> {
        if self.href.starts_with("http://") || self.href.starts_with("https://") || self.href.starts_with("ipfs://") || self.href.starts_with("ipns://") {
            return None;
        }

        if doc_path.last().map(|l| l=="index.html").unwrap_or(false) {
            doc_path = &doc_path[..doc_path.len() - 1];
        }

        let mut path = doc_path.to_vec();
        let mut href = self.href.as_str();

        if href.starts_with("../") {
            // Get back in the path
            while href.starts_with("../") {
                if path.is_empty() {
                    return None;
                }
                path.pop();
                href = &href[3..];
            }
        } else if href.starts_with('/') || href.starts_with("./") {
            // Build from root
            href = href.split_at(href.find('/').unwrap_or(href.len())).1;
            path.truncate(1);
        }

        path.extend(href.split('/').map(|p| p.to_string()));
        Some(path)
    }

    fn square_sizes(&self) -> Vec<usize> {
        let mut sizes = Vec::new();
        for size in self.sizes.split(' ') {
            let Some((first, second)) = size.split_once('x') else {continue};
            let Ok(first) = first.parse::<usize>() else {continue};
            let Ok(second) = second.parse::<usize>() else {continue};
            if first != second {continue}
            sizes.push(first);
        }
        sizes
    }

    fn best_square_size(&self) -> Option<usize> {
        let mut best_square_size = None;
        for size in self.square_sizes() {
            best_square_size = Some(match best_square_size {
                Some(old_size) => match (old_size >= 16, size >= 16) {
                    (true, true) => std::cmp::min(old_size, size),
                    (true, false) => old_size,
                    (false, true) => size,
                    (false, false) => std::cmp::max(old_size, size),
                }
                None => size,
            });
        }

        best_square_size
    }
}

impl DocumentResult {
    pub fn sort_paths(&mut self) {
        // TODO: sort using more advanced algorithm
        self.paths.sort_by(|a, b| b.first().map(|f| f.contains('.')).cmp(&a.first().map(|f| f.contains('.'))).then_with(|| b.len().cmp(&a.len())));
    }

    pub fn sort_favicons(&mut self) {
        self.favicons.sort_by_cached_key(|desc| {
            if desc.mime_type == "image/svg+xml" {
                return 16;
            }
            let best_size = desc.best_square_size().unwrap_or(500);
            if best_size < 16 {1000-best_size} else  {best_size}
        });
    }

    pub fn is_grouping_result(&self, query: &Query) -> bool {
        let title = match (&self.title, &self.h1) {
            (Some(title), _) => title,
            (_, Some(h1)) => h1,
            (None, None) => return false,
        };
        let title_words = title.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 3).map(|w| w.to_lowercase()).collect::<Vec<_>>();

        fn words_match_query(comp: &QueryComp, words: &[String]) -> bool {
            match comp {
                QueryComp::Word(word) => words.contains(word),
                QueryComp::Filter { .. } => true,
                QueryComp::Not(inner) => !words_match_query(inner, words),
                QueryComp::NAmong { n, among } => {
                    let mut matching = 0;
                    for inner in among {
                        if words_match_query(inner, words) {
                            matching += 1;
                            if matching >= *n {
                                return true;
                            }
                        }
                    }
                    false
                }
            }
        }

        words_match_query(&query.root, &title_words)
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

    pub fn format_best_href(&self) -> String {
        let best_path = match self.paths.first() {
            Some(f) => f.clone(),
            None => return format!("https://{}.ipfs.dweb.link/", self.cid),
        };

        match format_path_for_gateway(&best_path) {
            Some(addr) => addr,
            None => format!("https://{}.ipfs.dweb.link/", self.cid),
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
