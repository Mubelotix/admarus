use faster_pest::*;

use super::QueryComp;

#[derive(Parser)]
#[grammar = "common/src/query/query.pest"]
pub struct Parser {

}

fn build_comp(ident: IdentRef<Ident>) -> QueryComp {
    match ident.as_rule() {
        Rule::word_comp => {
            let word = ident.children().next().unwrap();
            let word = word.children().map(|c| c.as_str()).collect::<Vec<_>>().join("");
            QueryComp::Word(word)
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

fn parse_search_query(query: &str) -> Result<QueryComp, Error> {
    let idents = Parser::parse_query(query)?;
    Ok(build_comp(idents.root()))
}

#[test]
fn test() {
    let input = "word AND (word AND word) OR other AND 3(word, NOT(word2), word3) AND NOT word AND lang=en";
    let output = Parser::parse_query(input).map_err(|e| e.print(input)).unwrap();
    let field = output.into_iter().next().unwrap();
    println!("{:#?}", field);

    let input = "word AND test AND test AND 2(word, word, word) AND NOT(word) AND lang=en";
    let output = parse_search_query(input).unwrap();
    println!("{:#?}", output);

}
