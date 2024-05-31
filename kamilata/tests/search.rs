//! A test running an unorganized network of many nodes, giving them all documents and then searching for them.

mod common;
use common::*;

const NODE_COUNT: usize = 60;

#[tokio::test]
#[ignore]
async fn search() -> Result<(), Box<dyn std::error::Error>> {
    info!("Reading data...");
    let movies = get_movies();

    info!("Initializing clients...");
    let mut clients = Vec::new();
    for _ in 0..NODE_COUNT {
        let client = Client::init().await;
        clients.push(client);
    }

    let mut logger = ClientLogger::new();
    logger.with_peer_id(clients[0].peer_id());
    logger.activate();

    info!("Creating connections...");
    for i in 0..NODE_COUNT {
        for _ in 0..6 {
            let randint = rand::random::<usize>() % NODE_COUNT;
            let peer_id = clients[randint].peer_id();
            let addr = clients[randint].addr().to_owned();
            clients[i].swarm_mut().dial(DialOpts::peer_id(peer_id).addresses(vec![addr]).build()).unwrap();
        }
    }

    info!("Adding documents...");
    for (i, movies) in movies.as_slice().chunks((movies.len() as f64 / NODE_COUNT as f64).ceil() as usize).enumerate() {
        clients[i].store().insert_documents(movies).await;
    }

    info!("Launching clients...");
    let mut controlers = Vec::new();
    for client in clients {
        controlers.push(client.run());
        sleep(Duration::from_millis((20000/NODE_COUNT) as u64)).await; // We launch the network over a time period of 20 seconds so that they don't always update their filters at the same time.
    }

    info!("Start leeching...");
    for c in &controlers {
        c.leech_from_all().await;
    }

    info!("Waiting for the network to stabilize...");
    sleep(Duration::from_secs(2)).await;
    // FIXME: This is not enough time to let the filters propagate beyond the very first levels
    
    info!("Searching...");
    let results = controlers[0].search(["hunger"].as_slice()).await;
    let mut expected = 0;
    let mut missing = Vec::new();
    for movie in movies {
        if movie.words().contains(&"hunger".to_string()) {
            expected += 1;
            if !results.hits.iter().any(|(r,_)| *r==movie) {
                missing.push(movie);
            }
        }
    }
    if missing.len() as f32 > expected as f32 * 0.5 {
        panic!("Too many missing results:\n{missing:#?}");
    } else if !missing.is_empty() {
        warn!("Less than 50% results are missing so the test is still considered successful. Missing:\n{missing:#?}");
    } else {
        info!("All results are present");
    }

    Ok(())
}

