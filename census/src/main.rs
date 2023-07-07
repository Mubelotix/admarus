mod db;
mod record;
mod endpoints;
mod prelude;

pub use prelude::*;

#[actix_web::main]
async fn main() {
    println!("Hello, world!");
    
    let f1 = DB.shutdowner();
    let f2 = DB.drain_task();
    let f3 = DB.update_stats_task();
    let f4 = HttpServer::new(|| {
        App::new()
            .service(submit_record)
            .service(get_peers)
        })
        .bind(("0.0.0.0", 14364)).expect("Can not bind to address")
        .run();

    let futures: Vec<Pin<Box<dyn Future<Output = ()>>>> = vec![Box::pin(f1), Box::pin(f2), Box::pin(f3)];
    let fdb = futures::future::select_all(futures);
    let _ = futures::future::join(f4, fdb).await;
}
