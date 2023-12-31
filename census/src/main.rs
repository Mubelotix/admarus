mod db;
mod record;
mod endpoints;
mod prelude;
mod stats;

pub use prelude::*;

#[actix_web::main]
async fn main() {
    let f1 = DB.shutdowner();
    let f2 = DB.drain_task();
    let f3 = DB.update_stats_task();
    let f4 = HttpServer::new(|| {
        App::new()
            .service(submit_record)
            .service(get_peers)
            .service(get_stats)
        })
        .bind(("127.0.0.1", 14364)).expect("Can not bind to address")
        .run();

    println!("Census running!");
    let futures: Vec<Pin<Box<dyn Future<Output = ()>>>> = vec![Box::pin(f1), Box::pin(f2), Box::pin(f3)];
    let fdb = futures::future::select_all(futures);
    let _ = futures::future::join(f4, fdb).await;
}
