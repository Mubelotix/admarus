use super::*;


#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    /// Changes whether the node is visible to the others.
    SetVisibility(bool),

    /// Set metadata for the node.
    /// Metadata is a piece of data that will be provided to others along with contact information.
    /// This carries arbitrary information.
    /// It can be used to specify which peers are welcome to connect.
    SetMetadata(Vec<u8>),

    /// Requests the list of peers.
    GetPeers {
        /// If set, the list of peers will be filtered to only include those running the given protocol.
        protocol_version: Option<String>,
        /// If set, the list of peers will be filtered to only include those with the given agent version.
        agent_version: Option<String>,
        /// If set, the list of peers will be filtered to only include those supporting all the given protocols.
        protocols: Option<Vec<String>>,
        /// If set, the list of peers will be filtered to only include those with the exact given metadata.
        metadata: Option<Vec<u8>>,
        /// Maximum number of results to return.
        max_results: usize,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    /// The request was successful.
    Ok,

    /// The request failed.
    Error(String),

    /// The list of peers.
    Peers(HashMap<String, Info>),
}

#[allow(clippy::uninit_vec)]
async fn read_prefixed(stream: &mut Stream, max_payload_size: usize) -> Result<Vec<u8>, IoError> {
    let mut buf = [0; 4];
    stream.read_exact(&mut buf).await?;
    let len = u32::from_be_bytes(buf) as usize;
    if len > max_payload_size {
        return Err(IoError::new(std::io::ErrorKind::InvalidData, "payload too large"));
    }
    let mut buf = Vec::with_capacity(len);
    unsafe { buf.set_len(len) }
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn send_prefixed(stream: &mut Stream, data: &[u8]) -> Result<(), IoError> {
    let len = data.len();
    stream.write_all(&(len as u32).to_be_bytes()).await?;
    stream.write_all(data).await?;
    Ok(())
}

pub async fn server_task(remote_peer_id: PeerId, mut stream: Stream, db: Arc<Db>) -> Result<(), IoError> {
    let request = read_prefixed(&mut stream, db.config().request_max_payload_size).await?;
    let request: Request = serde_json::from_slice(&request)?;

    let response = match request {
        Request::SetVisibility(visibility) => {
            db.set_visibility(&remote_peer_id, visibility).await;
            Response::Ok
        },
        Request::SetMetadata(metadata) => {
            db.set_metadata(&remote_peer_id, metadata).await;
            Response::Ok
        },
        Request::GetPeers { protocol_version, agent_version, protocols, metadata, max_results } => {
            let mut peers = db.gen_list(protocol_version, agent_version, protocols, metadata).await;
            peers.truncate(max_results);
            peers.truncate(db.config().max_results);

            let mut final_list = HashMap::new();
            for (peer_id, info) in peers {
                final_list.insert(peer_id.to_string(), info);
            }

            Response::Peers(final_list)
        },
    };

    let response = serde_json::to_vec(&response)?;
    send_prefixed(&mut stream, &response).await?;

    Ok(())
}

async fn client_task_inner(request: Request, mut stream: Stream, db: Arc<Db>) -> Result<Response, IoError> {
    let request = serde_json::to_vec(&request)?;
    send_prefixed(&mut stream, &request).await?;

    let response = read_prefixed(&mut stream, db.config().response_max_payload_size).await?;
    let response: Response = serde_json::from_slice(&response)?;

    Ok(response)
}

pub async fn client_task(request: Request, replier: RequestReplier, stream: Stream, db: Arc<Db>) {
    let response = client_task_inner(request, stream, db).await;
    let _ = replier.send(response);
}
