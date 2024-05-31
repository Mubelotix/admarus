//! This module contains the handler of requests from remote peers.

use super::*;

pub(crate) async fn handle_request<const N: usize, S: Store<N>>(
    mut stream: KamInStreamSink<Stream>,
    db: Arc<Db<N, S>>,
    our_peer_id: PeerId,
    remote_peer_id: PeerId
) -> HandlerTaskOutput {
    let request = match stream.next().await {
        Some(Ok(request)) => request,
        Some(Err(e)) => {
            error!("{our_peer_id} Error while receiving request from {remote_peer_id}: {e}");
            return HandlerTaskOutput::None;
        },
        None => return HandlerTaskOutput::None,
    };

    debug!("{our_peer_id} Request from {remote_peer_id}: {request:?}");
    match request {
        RequestPacket::GetFilters(refresh_packet) => {
            let task = seed_filters(stream, refresh_packet, db, our_peer_id, remote_peer_id);
            HandlerTaskOutput::SetTask {
                tid: 1,
                task: HandlerTask { fut: Box::pin(task), name: "seed_filters" },
            }
        },
        RequestPacket::Search(search_packet) => {
            let query = match S::Query::from_bytes(&search_packet.query) {
                Ok(query) => Arc::new(query),
                Err(e) => {
                    error!("{our_peer_id} Error while parsing query from {remote_peer_id}: {e}");
                    return HandlerTaskOutput::None;
                },
            };

            // Send routes
            let mut routes = Vec::new();
            for (peer_id, match_scores) in db.search_routes(&query).await {
                let addresses: Vec<String> = db.get_addresses(&peer_id).await.into_iter().map(|a| a.to_string()).collect();
                if !addresses.is_empty() {
                    routes.push(Route {
                        match_scores,
                        peer_id: peer_id.into(),
                        addresses,
                    });
                }
            }
            let Ok(()) = stream.start_send_unpin(ResponsePacket::Routes(RoutesPacket(routes))) else {return HandlerTaskOutput::None};
            let Ok(()) = stream.flush().await else {return HandlerTaskOutput::None};
            trace!("{our_peer_id} Sent routes to {remote_peer_id}.");

            // Get results
            let (sender, mut receiver) = channel::<S::Result>(100);
            spawn(async move {
                let fut = db.store().search(Arc::clone(&query));
                let mut result_stream = fut.await;
                while let Some(result) = result_stream.next().await {
                    let Ok(()) = sender.send(result).await else {break};
                }
            });

            // Send results
            while let Some(result) = receiver.recv().await {
                let Ok(()) = stream.start_send_unpin(ResponsePacket::Result(ResultPacket(result.into_bytes()))) else {break};
                let Ok(()) = stream.flush().await else {break};
            }

            // Send search over
            let Ok(()) = stream.start_send_unpin(ResponsePacket::SearchOver) else {return HandlerTaskOutput::None};
            let Ok(()) = stream.flush().await else {return HandlerTaskOutput::None};

            HandlerTaskOutput::None
        },
        RequestPacket::Disconnect(_) => todo!(),
    }
}
