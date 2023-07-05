#![allow(dead_code)]

use crate::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Query {
    #[serde(flatten)]
    pub root: QueryComp,
}

impl Query {
    pub fn positive_terms(&self) -> Vec<&String> {
        self.root.positive_terms()
    }

    pub fn positive_filters(&self) -> Vec<(&String, &String)> {
        self.root.positive_filters()
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
            QueryComp::Filter { .. } => Vec::new(),
            QueryComp::Not(_) => Vec::new(),
            QueryComp::NAmong { among, .. } => among.iter().flat_map(|c| c.positive_terms()).collect::<Vec<_>>(),
        }
    }

    pub fn positive_filters(&self) -> Vec<(&String, &String)> {
        match self {
            QueryComp::Word(_) => Vec::new(),
            QueryComp::Filter { name, value } => vec![(name, value)],
            QueryComp::Not(_) => Vec::new(),
            QueryComp::NAmong { among, .. } => among.iter().flat_map(|c| c.positive_filters()).collect::<Vec<_>>(),
        }
    }
}
