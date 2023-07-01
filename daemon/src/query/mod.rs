mod parsing;
pub use parsing::*;
mod matching;
pub use matching::*;

use crate::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Query {
    #[serde(flatten)]
    root: QueryComp,
}

impl Query {
    pub fn positive_terms(&self) -> Vec<&String> {
        self.root.positive_terms()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryComp {
    // word
    Word(String),
    // name=value
    Filter {
        name: String,
        value: String,
    },
    // not(comp)
    Not(Box<QueryComp>),
    // n(comp, comp, comp)
    NAmong {
        n: usize,
        among: Vec<QueryComp>,
    },
}

impl QueryComp {
    pub fn positive_terms(&self) -> Vec<&String> {
        match self {
            QueryComp::Word(word) => vec![word],
            QueryComp::Filter { name, value } => Vec::new(),
            QueryComp::Not(comp) => Vec::new(),
            QueryComp::NAmong { among, .. } => among.iter().flat_map(|c| c.positive_terms()).collect::<Vec<_>>(),
        }
    }
}
