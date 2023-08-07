#![allow(dead_code)]

use crate::prelude::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Query {
    #[serde(flatten)]
    pub root: QueryComp,
}

impl Query {
    pub fn positive_terms(&self) -> Vec<&String> {
        self.root.positive_terms()
    }

    pub fn terms(&self) -> Vec<&String> {
        self.root.terms()
    }

    pub fn weighted_terms(&self) -> Vec<(String, f64)> {
        self.root.clone_only_words().map(|r| r.weighted_terms(1.0)).unwrap_or_default()
    }

    pub fn positive_filters(&self) -> Vec<(&String, &String)> {
        self.root.positive_filters()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    pub fn clone_only_words(&self) -> Option<QueryComp> {
        match self {
            QueryComp::Word(word) => Some(QueryComp::Word(word.clone())),
            QueryComp::Filter { .. } => None,
            QueryComp::Not(comp) => {
                let comp = comp.clone_only_words()?;
                Some(QueryComp::Not(Box::new(comp)))
            },
            QueryComp::NAmong { n, among } => {
                let mut n = *n;
                let mut new_among = Vec::new();
                for comp in among {
                    match comp.clone_only_words() {
                        Some(comp) => new_among.push(comp),
                        None => n = n.saturating_sub(1),
                    }
                }
                match n == 0 {
                    true => None,
                    false => Some(QueryComp::NAmong { n, among: new_among }),
                }
            },
        }
    }

    pub fn positive_terms(&self) -> Vec<&String> {
        match self {
            QueryComp::Word(word) => vec![word],
            QueryComp::Filter { .. } => Vec::new(),
            QueryComp::Not(_) => Vec::new(),
            QueryComp::NAmong { among, .. } => among.iter().flat_map(|c| c.positive_terms()).collect::<Vec<_>>(),
        }
    }

    pub fn terms(&self) -> Vec<&String> {
        match self {
            QueryComp::Word(word) => vec![word],
            QueryComp::Filter { .. } => Vec::new(),
            QueryComp::Not(comp) => comp.terms(),
            QueryComp::NAmong { among, .. } => among.iter().flat_map(|c| c.terms()).collect::<Vec<_>>(),
        }
    }

    pub fn weighted_terms(&self, weight: f64) -> Vec<(String, f64)> {
        match self {
            QueryComp::Word(word) => vec![(word.to_string(), weight)],
            QueryComp::Filter { .. } => panic!("QueryComp::weighted_terms() called on filter"),
            QueryComp::Not(_) => panic!("QueryComp::weighted_terms() called on not"),
            QueryComp::NAmong { among, .. } => among.iter().flat_map(|c| c.weighted_terms(weight/(among.len() as f64))).collect::<Vec<_>>(), // FIXME: handle 0
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
