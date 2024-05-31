//! These tests are for checking that documents are found in accordance with the priority settings.
//! In both tests, two documents are added to the network:
//!   - A perfectly matching document at a distance of 2
//!   - A partially matching document at a distance of 1
//! Depending on the priority, the order of the results should be different.

mod common;
use common::*;

async fn init_network() -> (Movie, Movie, ClientController, ClientController, ClientController, ClientController) {
    let doc1 = Movie {
        id: 0,
        title: String::from("Perfect match"),
        overview: String::from("This is the perfectly matching document"),
        genres: Vec::new(),
        poster: String::new(),
        release_date: 0,
    };
    let doc2 = Movie {
        id: 1,
        title: String::from("Partial match"),
        overview: String::from("This is the partially matching document"),
        genres: Vec::new(),
        poster: String::new(),
        release_date: 0,
    };

    //      ╱ 2 ─ 3
    //    1  
    //      ╲ 4

    let mut client1 = Client::init().await; // Connected to 2 and 4
    let mut client2 = Client::init().await; // Connected to 1 and 3
    let client3 = Client::init().await; // Distance from 1: 2
    let client4 = Client::init().await; // Distance from 1: 1

    let mut logger = ClientLogger::new();
    //logger.with_peer_id(client1.peer_id());
    logger.with_alias(client1.peer_id(), "client 1");
    logger.with_alias(client2.peer_id(), "client 2");
    logger.with_alias(client3.peer_id(), "client 3");
    logger.with_alias(client4.peer_id(), "client 4");
    logger.activate();

    client1.swarm_mut().dial(DialOpts::peer_id(client2.peer_id()).addresses(vec![client2.addr().to_owned()]).build()).unwrap();
    client1.swarm_mut().dial(DialOpts::peer_id(client4.peer_id()).addresses(vec![client4.addr().to_owned()]).build()).unwrap();
    client2.swarm_mut().dial(DialOpts::peer_id(client3.peer_id()).addresses(vec![client3.addr().to_owned()]).build()).unwrap();

    client3.store().insert_document(doc1.clone()).await;
    client4.store().insert_document(doc2.clone()).await;

    let c1 = client1.run();
    let c2 = client2.run();
    let c3 = client3.run();
    let c4 = client4.run();

    sleep(Duration::from_secs(1)).await;
    c1.leech_from(&c2).await;
    c1.leech_from(&c4).await;
    c2.leech_from(&c3).await;

    info!("Waiting for filters to propagate...");
    sleep(Duration::from_secs(20)).await;

    (doc1, doc2, c1, c2, c3, c4)
}

#[tokio::test]
async fn speed_priority() {
    let (doc1, doc2, controller1, _controller2, _controller3, _controller4) = init_network().await;

    info!("Searching with speed priority...");
    let results = controller1.search_with_config(
        ["perfectly", "matching"].as_slice(),
        SearchConfig::default().with_priority(SearchPriority::speed()).with_req_limit(1)
    ).await;
    let hits = results.hits.into_iter().map(|h| h.0).collect::<Vec<_>>();
    assert_eq!(hits, vec![doc2, doc1]);
}

#[tokio::test]
async fn relevance_priority() {
    let (doc1, doc2, controller1, _controller2, _controller3, _controller4) = init_network().await;

    info!("Searching with relevance priority...");
    let results = controller1.search_with_config(
        ["perfectly", "matching"].as_slice(),
        SearchConfig::default().with_priority(SearchPriority::relevance()).with_req_limit(1)
    ).await;
    let hits = results.hits.into_iter().map(|h| h.0).collect::<Vec<_>>();
    assert_eq!(hits, vec![doc1, doc2]);
}

// TODO: test for variable priority
