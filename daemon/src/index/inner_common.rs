use super::*;

impl DocumentIndexInner {
    pub fn add_ancestor(&mut self, cid: &String, name: String, folder_cid: &String) {
        let lcid = match self.cids.get_by_right(cid) {
            Some(lcid) => lcid.to_owned(),
            None => {
                let lcid = LocalCid(self.cid_counter);
                self.cid_counter += 1;
                self.cids.insert(lcid, cid.clone());
                self.folders.insert(lcid);
                lcid
            }
        };

        let ancestor_lcid = match self.cids.get_by_right(folder_cid) {
            Some(lcid) => lcid.to_owned(),
            None => {
                let lcid = LocalCid(self.cid_counter);
                self.cid_counter += 1;
                self.cids.insert(lcid, folder_cid.clone());
                lcid
            }
        };
        self.folders.insert(ancestor_lcid);

        self.ancestors.entry(lcid).or_default().insert(ancestor_lcid, name);
    }

    pub fn build_path(&self, cid: &String) -> Option<Vec<Vec<String>>> {
        let lcid = match self.cids.get_by_right(cid) {
            Some(lcid) => lcid.to_owned(),
            None => {
                warn!("Tried to build path for unknown cid: {cid}");
                return None;
            },
        };

        // List initial paths that will be explored
        let mut current_paths: Vec<(LocalCid, Vec<String>)> = Vec::new();
        for (ancestor, name) in self.ancestors.get(&lcid)? {
            current_paths.push((ancestor.to_owned(), vec![name.to_owned()]));
        }

        // Expand known paths and keep track of them all
        let mut paths: Vec<(LocalCid, Vec<String>)> = Vec::new();
        while let Some(current_path) = current_paths.pop() {
            if let Some(ancestors) = self.ancestors.get(&current_path.0) {
                for (ancestor, name) in ancestors {
                    if name.is_empty() {
                        continue;
                    }
                    let mut new_path = current_path.clone();
                    new_path.0 = ancestor.to_owned();
                    new_path.1.insert(0, name.to_owned());
                    current_paths.push(new_path);
                }
            }
            paths.push(current_path);
        }

        // Resolve the root cid to build final paths
        let mut final_paths = Vec::new();
        for (root, mut path) in paths {
            if let Some(first) = path.first() {
                if first.starts_with("dns-pin-") {
                    let dns_pin_with_suffix = first.split_at(8).1;
                    if let Some(i) = dns_pin_with_suffix.bytes().rposition(|c| c == b'-') {
                        let dns_pin = dns_pin_with_suffix.split_at(i).0;
                        let (domain, path_start) = dns_pin.split_once('/').unwrap_or((dns_pin, "/"));
                        let (domain, path_start) = (domain.to_owned(), path_start.to_owned());
                        path[0] = domain;
                        for path_part in path_start.split('/').rev() {
                            if !path_part.is_empty() {
                                path.insert(1, path_part.to_owned());
                            }
                        }
                        final_paths.push(path);
                        continue;
                    }
                }
            }
            let root_cid = match self.cids.get_by_left(&root) {
                Some(root_cid) => root_cid.to_owned(),
                None => match self.cids.get_by_left(&root) {
                    Some(root_cid) => root_cid.to_owned(),
                    None => continue,
                },
            };
            path.insert(0, root_cid);
            final_paths.push(path);
        }

        Some(final_paths)
    }
}
