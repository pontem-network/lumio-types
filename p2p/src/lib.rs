use eyre::WrapErr;
use futures::StreamExt;
use libp2p::multiaddr::Multiaddr;
use libp2p::{
    gossipsub::{self, IdentTopic},
    mdns,
    swarm::{Swarm, SwarmEvent},
};
use lumio_types::rpc::{AttributesArtifact, LumioEvents};

use std::hash::{Hash, Hasher};
use std::sync::LazyLock;
use std::time::Duration;

pub use jwt::JwtSecret;

mod jwt;

// We create a custom network behaviour that combines Gossipsub and Mdns.
#[derive(libp2p::swarm::NetworkBehaviour)]
struct LumioBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_on: Vec<Multiaddr>,
    pub bootstrap_addresses: Vec<Multiaddr>,
    pub jwt: JwtSecret,
}

enum SubscribeCommand {
    /// Events from op-move to lumio
    OpMove(futures::channel::mpsc::Sender<AttributesArtifact>),
    /// Events from op-sol to lumio
    OpSol(futures::channel::mpsc::Sender<AttributesArtifact>),
    /// Events from lumio to op-sol
    LumioOpSol(futures::channel::mpsc::Sender<LumioEvents>),
    /// Events from lumio to op-move
    LumioOpMove(futures::channel::mpsc::Sender<LumioEvents>),
}

enum SendEventCommand {
    /// Events from op-move to lumio
    OpMove(AttributesArtifact),
    /// Events from op-sol to lumio
    OpSol(AttributesArtifact),
    /// Events from lumio to op-sol
    LumioOpSol(LumioEvents),
    /// Events from lumio to op-move
    LumioOpMove(LumioEvents),
}

enum Command {
    Subscribe(SubscribeCommand),
    SendEvent(SendEventCommand),
}

pub struct Node {
    cmd_sender: futures::channel::mpsc::Sender<Command>,
}

pub struct NodeRunner {
    swarm: Swarm<LumioBehaviour>,
    jwt: JwtSecret,
    cmd_receiver: futures::channel::mpsc::Receiver<Command>,
}

static AUTH_TOPIC: LazyLock<IdentTopic> = LazyLock::new(|| IdentTopic::new("/lumio/v1/auth"));

impl Node {
    pub fn new(cfg: Config) -> eyre::Result<(Self, NodeRunner)> {
        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )?
            .with_quic()
            .with_behaviour(|key| {
                // To content-address message, we can take the hash of message and use it as an ID.
                let message_id_fn = |message: &gossipsub::Message| {
                    let mut s = std::hash::DefaultHasher::new();
                    message.data.hash(&mut s);
                    gossipsub::MessageId::from(s.finish().to_string())
                };

                // Set a custom gossipsub configuration
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
                    .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
                    .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
                    .build()?;

                // build a gossipsub network behaviour
                let mut gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )?;

                gossipsub.subscribe(&AUTH_TOPIC)?;

                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id(),
                )?;
                Ok(LumioBehaviour { gossipsub, mdns })
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        let Config {
            listen_on,
            bootstrap_addresses,
            jwt,
        } = cfg;

        for a in listen_on {
            swarm.listen_on(a).context("Failed to listen on address")?;
        }
        for a in bootstrap_addresses {
            swarm.dial(a).context("Failed to listen on address")?;
        }

        swarm
            .behaviour_mut()
            .gossipsub
            .publish(AUTH_TOPIC.clone(), jwt.claim()?)
            .context("Failed to auth")?;
        let (cmd_sender, cmd_receiver) = futures::channel::mpsc::channel(100);

        Ok((
            Self { cmd_sender },
            NodeRunner {
                swarm,
                jwt,
                cmd_receiver,
            },
        ))
    }
}

impl NodeRunner {
    // -> !
    pub async fn run(mut self) {
        // Kick it off
        let auth_topic_hash = AUTH_TOPIC.hash();
        loop {
            futures::select! {
                event = self.swarm.select_next_some() => match event {
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer_id, _multiaddr) in list {
                            tracing::debug!("mDNS discovered a new peer: {peer_id}");
                            self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        }
                    },
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                        for (peer_id, _multiaddr) in list {
                            tracing::debug!("mDNS discover peer has expired: {peer_id}");
                            self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        }
                    },
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        message: libp2p::gossipsub::Message {
                            data,
                            topic,
                            ..
                        },
                        ..
                    })) => {
                        match topic {
                            t if t == auth_topic_hash => {
                                self.jwt.decode(String::from_utf8(data).expect("FIXME")).expect("FIXME");
                            }
                            _ => unreachable!(),
                        }
                    },
                    SwarmEvent::NewListenAddr { address, .. } => {
                        tracing::debug!("Local node is listening on {address}");
                    }
                    _ => {}
                }
            }
        }
    }
}
