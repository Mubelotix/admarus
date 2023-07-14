use crate::prelude::*;

pub async fn manage_dns_pins(config: Arc<Args>) {
    if config.dns_pins.is_empty() {
        return;
    }
    if config.dns_pins.len() > 10 {
        warn!("You have a lot of DNS pins. Don't hesitate lowering the dns_pins_interval if you get rate limited by your DNS provider.")
    }
    let mut dns_pins_interval = config.dns_pins_interval;
    if dns_pins_interval < 60*3 {
        warn!("Your dns_pins_interval is too low. Increasing to 3 minutes.");
        dns_pins_interval = 60*3;
    }

    // Find old pins and look for the previous DNS pins
    let old_pins = match list_pinned(&config.ipfs_rpc).await {
        Ok(pins) => pins,
        Err(err) => {
            error!("Failed to list old DNS pins: {err}");
            Vec::new()
        }
    };
    let mut previous_dns_pins = Vec::new();
    for cid in old_pins {
        let dag = match get_dag(&config.ipfs_rpc, &cid).await {
            Ok(dag) => dag,
            Err(err) => {
                error!("Failed to get DAG {cid}: {err}");
                continue;
            }
        };
        let Some(serde_json::Value::Array(links)) = dag.get("Links") else {continue};

        if !links.is_empty() && links.iter().all(|link| {
            link.get("Name").and_then(|name| name.as_str()).map(|name| name.starts_with("dns-pin-")).unwrap_or(false)
        }) {
            previous_dns_pins.push(cid);
        }
    }

    loop {
        let start = Instant::now();

        // Resolve DNS pins
        trace!("Resolving {} DNS pins", config.dns_pins.len());
        let mut values = HashMap::new();
        for dns_pin in &config.dns_pins {
            let path = format!("/ipns/{dns_pin}");
            let cid = match resolve(&config.ipfs_rpc, &path).await {
                Ok(cid) => cid,
                Err(err) => {
                    error!("Failed to resolve DNS pin {dns_pin}: {err}");
                    continue;
                }
            };
            values.entry(dns_pin).or_insert_with(Vec::new).push(cid);
        }
        for values in values.values_mut() {
            values.sort();
        }
        if values.is_empty() {
            warn!("No DNS pins found");
            sleep(Duration::from_secs(dns_pins_interval)).await;
            continue;
        }

        // Add dag to IPFS
        trace!("Adding DAG with {} pins to IPFS", values.len());
        let mut dag_json = String::from(r#"{"Data":{"/":{"bytes":"CAE"}},"Links":["#);
        for (dns_pin, cids) in values {
            for (i, cid) in cids.iter().enumerate() {
                dag_json.push_str(&format!(r#"{{"Hash":{{"/":"{cid}"}},"Name":"dns-pin-{dns_pin}-{i}"}}"#));
            }
        }
        dag_json.push_str("]}");
        let cid = match put_dag(&config.ipfs_rpc, dag_json, true).await {
            Ok(cid) => cid,
            Err(err) => {
                error!("Failed to put DAG for DNS pins on IPFS: {err}");
                sleep(Duration::from_secs(dns_pins_interval)).await;
                continue;
            },
        };
        trace!("Added DNS-pins' DAG: ipfs://{cid}");

        // Replace old dag with new one
        if !(previous_dns_pins.len() == 1 && previous_dns_pins[0] == cid) {
            trace!("Pinning the new DAG");
            if let Err(e) = add_pin(&config.ipfs_rpc, &cid).await {
                error!("Failed to pin new DNS pins: {e}");
                sleep(Duration::from_secs(dns_pins_interval)).await;
                continue;
            }
        }

        // Remove old pins
        trace!("Removing old DNS pins"); 
        for old_pin in previous_dns_pins.into_iter().filter(|c| c!=&cid) {
            if old_pin == cid {
                continue
            }
            if let Err(e) = remove_pin(&config.ipfs_rpc, &old_pin).await {
                error!("Failed to remove old DNS pin {old_pin}: {e}");
            }
        }
        previous_dns_pins = vec![cid];

        trace!("Waiting for next DNS pins interval");
        sleep(Duration::from_secs(dns_pins_interval).saturating_sub(start.elapsed())).await;
    }
}
