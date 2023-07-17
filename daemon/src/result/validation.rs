use crate::prelude::*;

pub enum InvalidResult {
    InvalidCid(libipld::cid::Error),
    NoTitle,
    NoDesc,
    InvalidTermCounts,
}

impl DocumentResult {
    pub fn validate_no_fetch(mut self, query: &Query) -> Result<DocumentResult, InvalidResult> {
        // Validate cid and icon_cid
        if let Err(e) = Cid::try_from(self.cid.clone()) {
            warn!("Invalid CID for {}: {e}", self.cid);
            return Err(InvalidResult::InvalidCid(e));
        }
        if let Some(icon_cid) = self.icon_cid.clone() {
            if let Err(e) = Cid::try_from(icon_cid) {
                warn!("Invalid icon CID for {}: {e}", self.cid);
                self.icon_cid = None;
            }
        }

        // Validate paths
        let previous_len = self.paths.len();
        while self.paths.iter().map(|path| path.iter().map(|s| s.len()).sum::<usize>()).sum::<usize>() >= 10_000 {
            self.paths.pop();
        }
        if previous_len != self.paths.len() {
            warn!("Remove {} paths for {} to match the size limit of 10kB", previous_len - self.paths.len(), self.cid);
        }

        // Validate title and h1
        if let Some(title) = self.title.clone() {
            if title.len() > 1000 {
                warn!("Title too long for {}: {} bytes", self.cid, title.len());
                self.title = None;
            }
        }
        if let Some(h1) = self.h1.clone() {
            if h1.len() > 1000 {
                warn!("H1 too long for {}: {} bytes", self.cid, h1.len());
                self.h1 = None;
            }
        }
        if self.title.is_none() && self.h1.is_none() {
            warn!("No title or h1 for {}", self.cid);
            return Err(InvalidResult::NoTitle);
        }

        // Validate description and extract
        if let Some(description) = self.description.clone() {
            if description.len() > 5_000 {
                warn!("Description too long for {}: {} bytes", self.cid, description.len());
                self.description = None;
            }
        }
        if let Some(extract) = self.extract.clone() {
            if extract.len() > 5_000 {
                warn!("Extract too long for {}: {} bytes", self.cid, extract.len());
                self.extract = None;
            }
        }
        if self.description.is_none() && self.extract.is_none() {
            warn!("No description or extract for {}", self.cid);
            return Err(InvalidResult::NoDesc);
        }

        // Validate term_counts and word_count
        let positive_terms = query.positive_terms();
        if self.term_counts.len() != positive_terms.len() {
            warn!("Term counts length mismatch for {}: {} != {}", self.cid, self.term_counts.len(), positive_terms.len());
            return Err(InvalidResult::InvalidTermCounts);
        }
        
        // Validate common_words
        if let Some(common_words) = self.common_words {
            if !(0.0..=1.0).contains(&common_words) {
                warn!("Common words out of range for {}: {}", self.cid, common_words);
                self.common_words = None;
            }
        }

        Ok(self)
    }
}
