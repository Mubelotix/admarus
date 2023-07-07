use crate::prelude::*;

#[derive(Deserialize)]
struct ApiRecord {
    record: Record,
    public_key: Vec<u8>,
    signature: Vec<u8>,
}

#[post("/api/v0/submit")]
async fn submit_record(record: web::Json<ApiRecord>, req: HttpRequest) -> impl Responder {
    let ApiRecord { mut record, public_key, signature } = record.into_inner();

    #[cfg(feature = "debug_logs")]
    println!("Received record: {record:?}");

    // Check authenticity
    let public_key = match PublicKey::try_decode_protobuf(public_key.as_slice()) {
        Ok(key) => key,
        Err(err) => return HttpResponse::BadRequest().body(format!("Unable to decode public key: {err}")),
    };
    let peer_id = public_key.to_peer_id();
    if record.peer_id != peer_id.to_string() {
        return HttpResponse::Unauthorized().body(format!("Peer ID mismatch (record: {}, public key: {})", record.peer_id, peer_id));
    }
    let record_hash = record.hash();
    if !public_key.verify(&record_hash, signature.as_slice()) {
        return HttpResponse::Unauthorized().body(format!("Invalid signature (record hash: {record_hash:x})"));
    }

    // Check validity
    if record.addrs.is_empty() {
        return HttpResponse::BadRequest().body("No addresses provided");
    }
    if record.addrs.len() > 30 {
        return HttpResponse::BadRequest().body("Too many addresses provided (max 30)");
    }
    if record.addrs.iter().any(|addr| addr.len() >= 200) {
        return HttpResponse::BadRequest().body("Addr too long");
    }
    if record.folders.len() > 500 {
        return HttpResponse::BadRequest().body("Too many folders provided (max 500)");
    }
    let mut valid_folders = record.folders.into_iter()
        .filter_map(|(cid,c)| libipld::cid::Cid::try_from(cid.as_str()).ok().map(|cid| (cid,c)))
        .filter_map(|(cid,c)| cid.into_v1().ok().map(|cid| (cid,c)))
        .map(|(cid, c)| (cid.to_string(), c.clamp(0, 10_000_000)))
        .collect::<Vec<_>>();
    valid_folders.sort_by_cached_key(|(cid,_)| cid.clone());
    valid_folders.dedup_by_key(|(cid,_)| cid.clone());
    record.folders = valid_folders;

    let ip = req.peer_addr().map(|addr| addr.ip().to_string()).unwrap_or(String::from("Unknown"));
    DB.insert_record(record, ip).await;
    HttpResponse::Ok().body("Success!")
}

#[derive(Deserialize)]
struct GetPeersQuery {
    count: Option<usize>,
    exclude: Option<String>,
}

#[get("/api/v0/peers")]
async fn get_peers(query: web::Query<GetPeersQuery>) -> impl Responder {
    let count = query.count.unwrap_or(50);
    let count = std::cmp::min(count, 100);
    let mut exclude = query.exclude.as_ref().map(|e| e.split(',').map(String::from).collect::<Vec<_>>()).unwrap_or_default();
    exclude.truncate(50);
    let peers = DB.draw_peers(count, &exclude).await;
    HttpResponse::Ok().json(peers)
}

#[get("/api/v0/stats")]
async fn get_stats() -> impl Responder {
    let stats = DB.get_stats().await;
    HttpResponse::Ok().json(stats)
}
