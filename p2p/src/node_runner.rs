use futures::StreamExt;
use libp2p::{gossipsub, mdns, swarm::SwarmEvent, Swarm};
use lumio_types::rpc::{AttributesArtifact, LumioEvents};

use crate::{topics, Command, JwtSecret, LumioBehaviour, LumioBehaviourEvent, SubscribeCommand};

pub struct NodeRunner {
    swarm: Swarm<LumioBehaviour>,
    jwt: JwtSecret,
    cmd_receiver: futures::channel::mpsc::Receiver<Command>,

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
