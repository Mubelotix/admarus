use crate::prelude::*;
use libp2p::{swarm::{Swarm, SwarmBuilder}, identity::Keypair, PeerId, tcp, Transport, core::{transport::OrTransport, upgrade}, mplex::MplexConfig, noise::{NoiseConfig, self}};

const FILTER_SIZE: usize = 125000;

pub struct KamilataNode {
    swarm: Swarm<KamilataBehavior<FILTER_SIZE, DocumentIndex<FILTER_SIZE>>>,
}

impl KamilataNode {
    pub async fn init(index: DocumentIndex<FILTER_SIZE>) -> KamilataNode {
        let local_key = Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        let behaviour = KamilataBehavior::new_with_store(local_peer_id, index);
        
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::new());

        let transport = tcp_transport
            .upgrade(upgrade::Version::V1Lazy)
            .authenticate(
                noise::Config::new(&local_key).expect("Signing libp2p-noise static DH keypair failed."),
            )
            .multiplex(MplexConfig::default())
            .boxed();
        
        let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();
        swarm.listen_on("/ip4/127.0.0.1/tcp/4002".parse().unwrap()).unwrap();

        KamilataNode {
            swarm,
        }
    }
}
