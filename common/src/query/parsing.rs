use faster_pest::*;

use super::SearchQueryComp;

#[derive(Parser)]
#[grammar = "common/src/query/query.pest"]
pub struct Parser {

}

fn build_comp(ident: IdentRef<Ident>) -> SearchQueryComp {
    match ident.as_rule() {
        Rule::word_comp => {
            let word = ident.children().map(|c| c.as_str()).collect::<Vec<_>>().join("");
            SearchQueryComp::Word(word)
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
            SearchQueryComp::NAmong {
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
            SearchQueryComp::NAmong {
                n: 1,
                among: children.into_iter().map(build_comp).collect::<Vec<_>>(),
            }
        },
        Rule::not_comp => {
            let child = ident.children().next().unwrap();
            SearchQueryComp::Not(Box::new(build_comp(child)))
        },
        Rule::namong_comp => {
            let mut children = ident.children();
            let n = children.next().unwrap().as_str().parse::<usize>().unwrap();
            SearchQueryComp::NAmong {
                n,
                among: children.map(build_comp).collect::<Vec<_>>(),
            }
        },
        _ => todo!()
    }
}

fn parse_search_query(query: &str) -> Result<SearchQueryComp, Error> {
    let idents = Parser::parse_query(query)?;
    Ok(build_comp(idents.root()))
}

#[test]
fn test() {
    let input = "word AND (word AND word) OR other AND 3(word, NOT(word2), word3) AND NOT word";
    let output = Parser::parse_query(input).map_err(|e| e.print(input)).unwrap();
    let field = output.into_iter().next().unwrap();
    println!("{:#?}", field);

    let input = "word AND test AND test AND 2(word, word, word) AND NOT(word)";
    let output = parse_search_query(input).unwrap();
    println!("{:#?}", output);

}
