///! A test to make sure directives that from KamilataConfig (fields in_routing_peers and out_routing_peers) are respected.

mod common;
use common::*;

#[tokio::test]
async fn limits() -> Result<(), Box<dyn std::error::Error>> {
    let node_config = || KamilataConfig {
        protocol_names: vec![String::from("/kamilata/0.1.0")],
        max_seeders: 5,
        max_leechers: 5,
        get_filters_interval: MinTargetMax::new(60_000_000, 60_000_000, 60_000_000),
        filter_count: 0,
        approve_leecher: None,
    };

    info!("Initializing clients...");
    let main_client = Client::init_with_config(node_config()).await;
    let mut clients = Vec::new();
    for _ in 0..10 {
        let client = Client::init_with_config(node_config()).await;
        clients.push(client);
    }

    let mut logger = ClientLogger::new();
    logger.with_peer_id(main_client.peer_id());
    logger.activate();

    info!("Launching clients...");
    let main_controler = main_client.run();
    let mut controlers = Vec::new();
    let mut addresses = Vec::new();
    for client in clients {
        addresses.push(client.addr().clone());
        controlers.push(client.run());
    }

    info!("Creating connections with nodes...");
    for addr in addresses {
        main_controler.dial(addr.clone()).await;
    }
    sleep(Duration::from_secs(2)).await;

    // There should be no seeding nor leeching because we havn't instructed nodes to do so yet
    let (seeders, leechers) = main_controler.get_routing_stats().await;
    assert_eq!(seeders, 0);
    assert_eq!(leechers, 0);

    info!("Start leeching...");
    for c in &controlers {
        main_controler.leech_from(c).await;
    }
    sleep(Duration::from_secs(2)).await;

    // Seeders should be capped at 5
    let (seeders, leechers) = main_controler.get_routing_stats().await;
    assert_eq!(seeders, 5);
    assert_eq!(leechers, 0);

    info!("Start seeding...");
    for c in &controlers {
        c.leech_from(&main_controler).await;
    }
    sleep(Duration::from_secs(2)).await;

    // Leechers should be capped at 5
    let (seeders, leechers) = main_controler.get_routing_stats().await;
    assert_eq!(seeders, 5);
    assert_eq!(leechers, 5);

    Ok(())
}

