use eyre::WrapErr;
use futures::StreamExt;
use libp2p::multiaddr::Multiaddr;
use libp2p::{
    gossipsub::{self, IdentTopic},
    mdns,
    swarm::SwarmEvent,
};

use std::hash::{Hash, Hasher};
use std::sync::LazyLock;
use std::time::Duration;

// We create a custom network behaviour that combines Gossipsub and Mdns.
#[derive(libp2p::swarm::NetworkBehaviour)]
struct LumioBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

pub struct Config {
    listen_on: Vec<Multiaddr>,
    bootstrap_addresses: Vec<Multiaddr>,
}

pub struct Node {}

static AUTH_TOPIC: LazyLock<IdentTopic> = LazyLock::new(|| IdentTopic::new("/lumio/v1/auth"));

impl Node {
    pub async fn spawn(cfg: Config) -> eyre::Result<Self> {
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
                let gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )?;

                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id(),
                )?;
                Ok(LumioBehaviour { gossipsub, mdns })
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        // subscribes to our topic
        swarm.behaviour_mut().gossipsub.subscribe(&AUTH_TOPIC)?;

        // Listen on all interfaces and whatever port the OS assigns
        swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

        swarm
            .behaviour_mut()
            .gossipsub
            .publish(AUTH_TOPIC.clone(), todo!("JWT") as Vec<u8>)
            .context("Failed to auth")?;

        // Kick it off
        let auth_topic_hash = AUTH_TOPIC.hash();
        loop {
            futures::select! {
                event = swarm.select_next_some() => match event {
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer_id, _multiaddr) in list {
                            tracing::debug!("mDNS discovered a new peer: {peer_id}");
                            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        }
                    },
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                        for (peer_id, _multiaddr) in list {
                            tracing::debug!("mDNS discover peer has expired: {peer_id}");
                            swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        }
                    },
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        propagation_source: peer_id,
                        message_id: id,
                        message: libp2p::gossipsub::Message {
                            data,
                            topic,
                            ..
                        },
                    })) => {
                        match topic {
                            t if t == auth_topic_hash => todo!("Auth"),
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
