use crate::prelude::*;

#[derive(Deserialize)]
struct ApiRecord {
    record: Record,
    public_key: Vec<u8>,
    signature: Vec<u8>,
}

#[post("/api/v0/submit")]
async fn submit_record(record: web::Json<ApiRecord>, req: HttpRequest) -> impl Responder {
    let ApiRecord { record, public_key, signature } = record.into_inner();

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

#[derive(Clone, Default, Serialize)]
pub  struct NetworkStats {
    peers: usize,
    documents: usize,
    different_documents: usize,
    median_documents_per_peer: usize,
    // TODO: different_queries: usize,
    // TODO: health: f64,
}

#[derive(Clone, Default, Serialize)]
pub struct GetStatsResp {
    stats_1h: NetworkStats,
    prev_stats_1h: NetworkStats,
    stats_24h: NetworkStats,
    prev_stats_24h: NetworkStats,
}

#[get("/api/v0/stats")]
async fn get_stats() -> impl Responder {
    let stats = DB.get_stats().await;
    HttpResponse::Ok().json(stats)
}
