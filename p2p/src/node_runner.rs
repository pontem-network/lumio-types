use eyre::{Result, WrapErr};
use futures::StreamExt;
use libp2p::{gossipsub, mdns, swarm::SwarmEvent, PeerId, Swarm};
use lumio_types::rpc::{AttributesArtifact, LumioEvents};

use std::collections::HashSet;

use crate::{topics, Command, JwtSecret, LumioBehaviour, LumioBehaviourEvent, SubscribeCommand};

pub struct NodeRunner {
    swarm: Swarm<LumioBehaviour>,
    jwt: JwtSecret,
    cmd_receiver: futures::channel::mpsc::Receiver<Command>,

    authorized: HashSet<PeerId>,

    op_move_events: Option<futures::channel::mpsc::Sender<AttributesArtifact>>,
    op_sol_events: Option<futures::channel::mpsc::Sender<AttributesArtifact>>,
    lumio_sol_events: Option<futures::channel::mpsc::Sender<LumioEvents>>,
    lumio_move_events: Option<futures::channel::mpsc::Sender<LumioEvents>>,
}

impl NodeRunner {
    pub(crate) fn new(
        swarm: Swarm<LumioBehaviour>,
        jwt: JwtSecret,
        cmd_receiver: futures::channel::mpsc::Receiver<Command>,
    ) -> Self {
        Self {
            swarm,
            jwt,
            cmd_receiver,

            authorized: Default::default(),
            op_move_events: None,
            op_sol_events: None,
            lumio_sol_events: None,
            lumio_move_events: None,
        }
    }

    // -> !
    pub async fn run(mut self) {
        // Kick it off
        let auth_topic_hash = topics::AUTH.hash();
        loop {
            futures::select! {
                cmd = self.cmd_receiver.next() => {
                    // If no listeners then we exit
                    let Some(cmd) = cmd else { return; };

                    match cmd {
                        Command::Subscribe(cmd) => {
                            self.swarm
                                .behaviour_mut()
                                .gossipsub
                                .subscribe(cmd.topic())
                                .expect("FIXME");
                            match cmd {
                                SubscribeCommand::OpMove(sender) => self.op_move_events = Some(sender),
                                SubscribeCommand::OpSol(sender) => self.op_sol_events = Some(sender),
                                SubscribeCommand::LumioOpSol(sender) => self.lumio_sol_events = Some(sender),
                                SubscribeCommand::LumioOpMove(sender) => self.lumio_move_events = Some(sender),
                            }
                        }
                        _ => todo!(),
                    }
                },
                event = self.swarm.select_next_some() => match event {
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer_id, multiaddr) in list {
                            tracing::debug!(?peer_id, %multiaddr, "mDNS discovered a new peer");
                            self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        }

                        // AUTH for as there are new peers
                        let claim = self.jwt.claim().expect("Encoding JWT never fails");
                        let result = self.swarm
                            .behaviour_mut()
                            .gossipsub
                            .publish(topics::AUTH.clone(), claim);
                        if let Err(err) = result {
                            tracing::debug!(?err, "Failed to send auth message because of new peer");
                        }
                    },
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                        for (peer_id, multiaddr) in list {
                            tracing::debug!(?peer_id, %multiaddr, "mDNS discover peer has expired");
                            self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        }
                    },
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        message: libp2p::gossipsub::Message {
                            data,
                            topic,
                            source,
                            ..
                        },
                        ..
                    })) => {
                        let Some(source) = source else {
                            tracing::debug!(?topic, "Ignoring message as sender is unknown");
                            continue
                        };

                        match (topic, self.authorized.contains(&source)) {
                            (t, _) if t == auth_topic_hash => {
                                let Err(err) = self.authorize(source, data) else { continue };
                                tracing::debug!("Failed to auth peer {source}: {err:?}");
                            }
                            (topic, _) => tracing::debug!(?topic, "Ignoring message from unknown topic"),
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

    pub fn authorize(&mut self, source: PeerId, data: Vec<u8>) -> Result<()> {
        self.jwt
            .decode(String::from_utf8(data).context("Failed to decode JWT claim. Invalid UTF8")?)
            .context("Failed to decode JWT claim")?;
        self.authorized.insert(source);
        Ok(())
    }
}
