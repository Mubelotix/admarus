//! This tasks sends a request through the handler and reports the response to a channel.

use super::*;

pub async fn request<const N: usize>(
    mut stream: KamOutStreamSink<Stream>,
    request: RequestPacket,
    sender: OneshotSender<Option<ResponsePacket>>,
    our_peer_id: PeerId,
    remote_peer_id: PeerId
) -> HandlerTaskOutput {
    trace!("{our_peer_id} Sending request to {remote_peer_id}: {request:?}");

    // Send request packet
    match stream.start_send_unpin(request) {
        Ok(()) => (),
        Err(e) => {
            warn!("{our_peer_id} Error while sending request to {remote_peer_id}: {e}");
            let _ = sender.send(None);
            return HandlerTaskOutput::None;
        }
    }
    if stream.flush().await.is_err() {
        warn!("{our_peer_id} Error while sending request to {remote_peer_id}: flush failed");
        let _ = sender.send(None);
        return HandlerTaskOutput::None;
    }

    // Receive response packet
    let packet = match stream.next().await {
        Some(Ok(packet)) => packet,
        w => {
            warn!("{our_peer_id} Error while receiving response from {remote_peer_id}: stream closed {w:?}");
            let _ = sender.send(None);
            return HandlerTaskOutput::None;
        }
    };

    // Send results packet
    let _ = sender.send(Some(packet));
    
    HandlerTaskOutput::None
}

pub fn request_boxed<const N: usize>(
    stream: KamOutStreamSink<Stream>,
    vals: Box<dyn Any + Send>
) -> Pin<Box<dyn Future<Output = HandlerTaskOutput> + Send>> {
    let vals: Box<(RequestPacket, OneshotSender<Option<ResponsePacket>>, PeerId, PeerId)> = vals.downcast().unwrap(); // TODO: downcast unchecked?
    request::<N>(stream, vals.0, vals.1, vals.2, vals.3).boxed()
}

pub fn pending_request<const N: usize>(
    request: RequestPacket,
    sender: OneshotSender<Option<ResponsePacket>>,
    our_peer_id: PeerId,
    remote_peer_id: PeerId,
) -> PendingHandlerTask<Box<dyn Any + Send>> {
    PendingHandlerTask {
        params: Box::new((request, sender, our_peer_id, remote_peer_id)),
        fut: request_boxed::<N>,
        name: "request",
    }
}

