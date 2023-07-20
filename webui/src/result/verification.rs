use crate::prelude::*;

impl DocumentResult {
    pub fn agrees_with(&self, trusted: &DocumentResult) -> bool {
        self.cid == trusted.cid
            && (self.icon_cid.is_none() || self.icon_cid == trusted.icon_cid) // TODO: remove none here
            // TODO self.paths
            && self.title == trusted.title
            && (self.h1.is_none() || self.h1 == trusted.h1 || trusted.h1.is_none())
            && self.description == trusted.description
            && (self.extract.is_none() || self.extract == trusted.extract || trusted.extract.is_none())
            && self.term_counts == trusted.term_counts
            && self.word_count == trusted.word_count
            && (self.common_words.is_none() || self.common_words == trusted.common_words || trusted.common_words.is_none())
    }
}
