use crate::prelude::*;

const LUCKY_QUERIES: &[&str] = &["ipfs", "rust language", "bitcoin", "blog", "founder", "libp2p", "filecoin", "protocol labs", "peer to peer", "github"];

pub fn get_lucky_query(rng: Option<u64>) -> &'static str {
    let rng = match rng {
        Some(rng) => rng,
        None => {
            let mut buf = [0u8; 8];
            web_sys::window().unwrap().crypto().unwrap().get_random_values_with_u8_array(&mut buf).unwrap();
            u64::from_le_bytes(buf)
        }
    };

    let idx = (rng % LUCKY_QUERIES.len() as u64) as usize;
    LUCKY_QUERIES[idx]
}

impl Page {
    pub fn lucky_query(rng: Option<u64>) -> Self {
        Self::Results(Rc::new(get_lucky_query(rng).to_string()))
    }
}
