mod common;
use common::*;

/*
#[tokio::test]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client1 = Client::init(1000).await;
    client1.behaviour().insert_documents(vec![
        Movie {
            cid: "V for Vendetta".to_string(),
            desc: "In a future British dystopian society, a shadowy freedom fighter, known only by the alias of \"V\", plots to overthrow the tyrannical government - with the help of a young woman.".to_string(),
        },
        Movie {
            cid: "The Matrix".to_string(),
            desc: "When a beautiful stranger leads computer hacker Neo to a forbidding underworld, he discovers the shocking truth--the life he knows is the elaborate deception of an evil cyber-intelligence.".to_string(),
        },
        Movie {
            cid: "Revolution of Our Times".to_string(),
            desc: "Due to political restrictions in Hong Kong, this documentary following protestors since 2019, is broken into pieces, each containing interviews and historical context of the conflict.".to_string(),
        },
        Movie {
            cid: "The Social Dilemma".to_string(),
            desc: "Explores the dangerous human impact of social networking, with tech experts sounding the alarm on their own creations.".to_string(),
        },
        Movie {
            cid: "The Hunger Games".to_string(),
            desc: "Katniss Everdeen voluntarily takes her younger sister's place in the Hunger Games: a televised competition in which two teenagers from each of the twelve Districts of Panem are chosen at random to fight to the death.".to_string(),
        }
    ]).await;

    let addr = client1.addr().clone();
    let client1 = client1.run();

    let mut client2 = Client::init(1001).await;
    client2.swarm_mut().dial(addr).unwrap();
    let client2 = client2.run();

    sleep(Duration::from_secs(5)).await;

    let results = client2.search("Hunger").await;
    assert!(results.len() == 1);

    Ok(())
}
*/