//! This tasks returns its stream through a channel.

use super::*;

pub(crate) async fn search_req<const N: usize, S: Store<N>>(
    mut stream: KamOutStreamSink<Stream>,
    query: Arc<S::Query>,
    routes_sender: Sender<Vec<Route>>,
    result_sender: OngoingSearchFollower<N, S>,
    over_notifier: OneshotSender<()>,
    our_peer_id: PeerId,
    remote_peer_id: PeerId
) -> HandlerTaskOutput {
    trace!("{our_peer_id} Searching {remote_peer_id}");

    let request = RequestPacket::Search(SearchPacket { query: query.to_bytes() }); // TODO: remove conversion
    if let Err(e) = stream.start_send_unpin(request) {
        error!("{our_peer_id} Could not send search packet to {remote_peer_id}: {e}");
        return HandlerTaskOutput::None;
    }
    if let Err(e) = stream.flush().await {
        error!("{our_peer_id} Could not send flush search packet to {remote_peer_id}: {e}");
        return HandlerTaskOutput::None;
    }

    // Get routes
    let routes = match stream.next().await { // TODO: we should wait for the end of this function to use routes
        Some(Ok(ResponsePacket::Routes(RoutesPacket(routes)))) => routes,
        _ => {
            error!("{our_peer_id} Failed to receive response from {remote_peer_id}");
            return HandlerTaskOutput::None;
        }
    };
    debug!("{our_peer_id} Received {} routes from {remote_peer_id}", routes.len());
    let Ok(()) = routes_sender.send(routes).await else {return HandlerTaskOutput::None};

    // Get results
    let mut result_count = 0;
    let start = Instant::now();
    loop {
        if start.elapsed() > Duration::from_secs(20) {
            warn!("{our_peer_id} Search takes too long {remote_peer_id}");
            break;
        }
        match stream.next().await {
            Some(Ok(ResponsePacket::Result(ResultPacket(result)))) => {
                match S::Result::from_bytes(&result) {
                    Ok(result) => {
                        result_count += 1;
                        if let Err(e) = result_sender.send((result, remote_peer_id)).await {
                            warn!("{our_peer_id} results are dropping: {e}");
                            break;
                        }
                    },
                    Err(e) => warn!("{our_peer_id} Failed to deserialize result from {remote_peer_id}: {e}"),
                }
            },
            Some(Ok(ResponsePacket::SearchOver)) => {
                break;
            }
            _ => {
                error!("{our_peer_id} Failed to receive result from {remote_peer_id}");
                break;
            }
        }
    }

    debug!("{our_peer_id} Received {} results from {remote_peer_id}", result_count);
    
    let _ = over_notifier.send(());
    HandlerTaskOutput::None
}

pub(crate) fn search_req_boxed<const N: usize, S: Store<N>>(
    stream: KamOutStreamSink<Stream>,
    vals: Box<dyn Any + Send>
) -> Pin<Box<dyn Future<Output = HandlerTaskOutput> + Send>> {
    let vals: Box<(Arc<S::Query>, Sender<Vec<Route>>, OngoingSearchFollower<N, S>, OneshotSender<()>, PeerId, PeerId)> = vals.downcast().unwrap(); // TODO: downcast unchecked?
    search_req::<N, S>(stream, vals.0, vals.1, vals.2, vals.3, vals.4, vals.5).boxed()
}

pub(crate) fn pending_search_req<const N: usize, S: Store<N>>(
    query: Arc<S::Query>,
    routes_sender: Sender<Vec<Route>>,
    result_sender: OngoingSearchFollower<N, S>,
    over_notifier: OneshotSender<()>,
    our_peer_id: PeerId,
    remote_peer_id: PeerId
) -> PendingHandlerTask<Box<dyn Any + Send>> {
    PendingHandlerTask {
        params: Box::new((query, routes_sender, result_sender, over_notifier, our_peer_id, remote_peer_id)),
        fut: search_req_boxed::<N, S>,
        name: "search_req",
    }
}

