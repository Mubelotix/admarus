mod parsing;
pub use parsing::*;

pub struct SearchQuery {
    root: SearchQueryComp,
}

#[derive(Debug)]
pub enum SearchQueryComp {
    // word
    Word(String),
    // name=value
    Filter {
        name: String,
        value: String,
    },
    // not(comp)
    Not(Box<SearchQueryComp>),
    // n(comp, comp, comp)
    NAmong {
        n: usize,
        among: Vec<SearchQueryComp>,
    },
}
