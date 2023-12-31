use faster_pest::*;
use crate::prelude::*;

#[derive(faster_pest::Parser)]
#[grammar = "daemon/src/query/query.pest"]
pub struct Parser {

}

pub(super) fn build_comp(ident: IdentRef<Ident>) -> QueryComp {
    match ident.as_rule() {
        Rule::word_comp => {
            let word = ident.children().next().unwrap();
            let word = word.children().map(|c| c.as_str()).collect::<Vec<_>>().join("");
            QueryComp::Word(word.to_lowercase()) // TODO: normalization (cf meilisearch)
        },
        Rule::and_comp => {
            let mut children = ident.children().collect::<Vec<_>>();
            let mut i = 0;
            while i < children.len() {
                if children[i].as_rule() == Rule::and_comp {
                    let child = children.remove(i);
                    children.extend(child.children());
                } else {
                    i += 1;
                }
            }
            QueryComp::NAmong {
                n: children.len(),
                among: children.into_iter().map(build_comp).collect::<Vec<_>>(),
            }
        },
        Rule::or_comp => {
            let mut children = ident.children().collect::<Vec<_>>();
            let mut i = 0;
            while i < children.len() {
                if children[i].as_rule() == Rule::or_comp {
                    let child = children.remove(i);
                    children.extend(child.children());
                } else {
                    i += 1;
                }
            }
            QueryComp::NAmong {
                n: 1,
                among: children.into_iter().map(build_comp).collect::<Vec<_>>(),
            }
        },
        Rule::quick_or_comp => {
            let words = ident.children().map(|c| 
                c.children().map(|c| c.as_str()).collect::<Vec<_>>().join("").to_lowercase()
            );
            QueryComp::NAmong {
                n: 1,
                among: words.map(QueryComp::Word).collect::<Vec<_>>(),
            }
        },
        Rule::not_comp => {
            let child = ident.children().next().unwrap();
            QueryComp::Not(Box::new(build_comp(child)))
        },
        Rule::namong_comp => {
            let mut children = ident.children();
            let n = children.next().unwrap().as_str().parse::<usize>().unwrap();
            QueryComp::NAmong {
                n,
                among: children.map(build_comp).collect::<Vec<_>>(),
            }
        },
        Rule::filter_comp => {
            let mut children = ident.children();
            let name = children.next().unwrap().children().map(|c| c.as_str()).collect::<Vec<_>>().join("");
            let value = children.next().unwrap().children().map(|c| c.as_str()).collect::<Vec<_>>().join("");
            QueryComp::Filter {
                name,
                value,
            }
        }
        _ => unreachable!()
    }
}

impl Query {
    pub fn parse(query: &str) -> Result<Query, Error> {
        let idents = Parser::parse_query(query)?;
        Ok(Query {
            root: build_comp(idents.root())
        })
    }
}

#[test]
fn test() {
    let input = "word AND (word AND word) OR other AND 3(word, NOT(word2), word3) AND NOT word AND lang=en";
    let output = Parser::parse_query(input).map_err(|e| e.print(input)).unwrap();
    let field = output.into_iter().next().unwrap();
    println!("{:#?}", field);

    let input = "word AND test AND test AND 2(word, word, word) AND NOT(word) AND lang=en";
    let output = Query::parse(input).unwrap_or_else(|e| {e.print(input); panic!()});
    println!("{:#?}", output);

    let input = "chloe helloco";
    let output = Query::parse(input).unwrap_or_else(|e| {e.print(input); panic!()});
    println!("{:#?}", output);
}
