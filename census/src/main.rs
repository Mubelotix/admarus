mod db;
mod record;
mod endpoints;
mod prelude;

pub use prelude::*;

#[actix_web::main]
async fn main() {
    println!("Hello, world!");
    
    let f1 = DB.shutdowner();
    let f2 = DB.run();
    let f3 = HttpServer::new(|| {
        App::new()
            .service(submit_record)
            .service(get_peers)
        })
        .bind(("0.0.0.0", 14364)).expect("Can not bind to address")
        .run();

    let fdb = futures::future::select(Box::pin(f1), Box::pin(f2));
    let _ = futures::future::join(f3, fdb).await;
}
