use futures::StreamExt;
use libp2p::{gossipsub, mdns, swarm::SwarmEvent, Swarm};

use crate::{Command, JwtSecret, LumioBehaviour, LumioBehaviourEvent, topics};

pub struct NodeRunner {
    pub(crate) swarm: Swarm<LumioBehaviour>,
    pub(crate) jwt: JwtSecret,
    pub(crate) cmd_receiver: futures::channel::mpsc::Receiver<Command>,
}

impl NodeRunner {
    // -> !
    pub async fn run(mut self) {
        // Kick it off
        let auth_topic_hash = topics::AUTH.hash();
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
