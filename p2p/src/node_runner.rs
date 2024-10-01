use eyre::{Result, WrapErr};
use futures::prelude::*;
use libp2p::{gossipsub, mdns, swarm::SwarmEvent, PeerId, Swarm};
use lumio_types::p2p::{SlotArtifact, SlotAttribute};
use serde::{Deserialize, Serialize};

use std::collections::HashSet;

use crate::topics::Topic;
use crate::{
    topics, Command, JwtSecret, LumioBehaviour, LumioBehaviourEvent, SendEventCommand,
    SubscribeCommand,
};

pub struct NodeRunner {
    swarm: Swarm<LumioBehaviour>,
    jwt: JwtSecret,
    cmd_receiver: tokio::sync::mpsc::Receiver<Command>,

    authorized: HashSet<PeerId>,

    op_move_events: Option<tokio::sync::mpsc::Sender<SlotArtifact>>,
    op_sol_events: Option<tokio::sync::mpsc::Sender<SlotArtifact>>,
    lumio_sol_events: Option<tokio::sync::mpsc::Sender<SlotAttribute>>,
    lumio_move_events: Option<tokio::sync::mpsc::Sender<SlotAttribute>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Auth {
    peer_id: PeerId,
    claim: String,
}

impl NodeRunner {
    pub(crate) fn new(
        swarm: Swarm<LumioBehaviour>,
        jwt: JwtSecret,
        cmd_receiver: tokio::sync::mpsc::Receiver<Command>,
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

    fn handle_command(&mut self, cmd: Command) {
        let (topic, data) = match cmd {
            Command::Subscribe(cmd) => {
                self.swarm
                    .behaviour_mut()
                    .gossipsub
                    .subscribe(cmd.topic())
                    .expect(
                        "Something went terribly wrong, as we failed to subscribe to some topic",
                    );
                match cmd {
                    SubscribeCommand::OpMove(sender) => self.op_move_events = Some(sender),
                    SubscribeCommand::OpSol(sender) => self.op_sol_events = Some(sender),
                    SubscribeCommand::LumioOpSol(sender) => self.lumio_sol_events = Some(sender),
                    SubscribeCommand::LumioOpMove(sender) => self.lumio_move_events = Some(sender),
                }
                return;
            }
            Command::SendEvent(SendEventCommand::OpMove(art)) => {
                (topics::OpMoveEvents::hash(), bincode::serialize(&art))
            }
            Command::SendEvent(SendEventCommand::OpSol(art)) => {
                (topics::OpSolEvents::hash(), bincode::serialize(&art))
            }
            Command::SendEvent(SendEventCommand::LumioOpSol(ev)) => {
                (topics::LumioSolEvents::hash(), bincode::serialize(&ev))
            }
            Command::SendEvent(SendEventCommand::LumioOpMove(ev)) => {
                (topics::LumioMoveEvents::hash(), bincode::serialize(&ev))
            }
        };

        match self.swarm.behaviour_mut().gossipsub.publish(
            topic.clone(),
            data.expect("bincode serialization never fails"),
        ) {
            // We don't care if there no peers
            Ok(_) | Err(gossipsub::PublishError::InsufficientPeers) => (),
            Err(err) => panic!("Failed to publish message: {err}"),
        }
    }

    async fn handle_message(&mut self, msg: libp2p::gossipsub::Message) {
        let libp2p::gossipsub::Message {
            data,
            topic,
            source,
            ..
        } = msg;
        let Some(source) = source else {
            tracing::debug!(?topic, "Ignoring message as sender is unknown");
            return;
        };

        match (topic, self.authorized.contains(&source)) {
            (t, _) if t == *topics::Auth::hash() => {
                let Ok(Auth { peer_id, claim }) = bincode::deserialize(&data) else {
                    tracing::debug!("Failed to decode op move event. Skipping...");
                    return;
                };

                let Err(err) = self.authorize(peer_id, claim) else {
                    return;
                };
                tracing::debug!("Failed to auth peer {source}: {err:?}");
            }
            (t, true) if t == *topics::OpMoveEvents::hash() => {
                let ch = self
                    .op_move_events
                    .as_mut()
                    .expect("We should always have a channel if we subscribed to topic");
                let Ok(msg) = bincode::deserialize(&data) else {
                    tracing::debug!("Failed to decode op move event. Skipping...");
                    return;
                };

                let _ = ch.send(msg).await;
            }
            (t, true) if t == *topics::OpSolEvents::hash() => {
                let ch = self
                    .op_sol_events
                    .as_mut()
                    .expect("We should always have a channel if we subscribed to topic");
                let Ok(msg) = bincode::deserialize(&data) else {
                    tracing::debug!("Failed to decode op sol event. Skipping...");
                    return;
                };

                let _ = ch.send(msg).await;
            }
            (t, true) if t == *topics::LumioSolEvents::hash() => {
                let ch = self
                    .lumio_sol_events
                    .as_mut()
                    .expect("We should always have a channel if we subscribed to topic");
                let Ok(msg) = bincode::deserialize(&data) else {
                    tracing::debug!("Failed to decode lumio sol event. Skipping...");
                    return;
                };

                let _ = ch.send(msg).await;
            }
            (t, true) if t == *topics::LumioMoveEvents::hash() => {
                let ch = self
                    .lumio_move_events
                    .as_mut()
                    .expect("We should always have a channel if we subscribed to topic");
                let Ok(msg) = bincode::deserialize(&data) else {
                    tracing::debug!("Failed to decode lumio move event. Skipping...");
                    return;
                };

                let _ = ch.send(msg).await;
            }
            (_, false) => tracing::trace!(?source, "Ignoring message from unauthorized"),
            (topic, true) => tracing::debug!(?topic, "Ignoring message from unknown topic"),
        }
    }

    #[tracing::instrument(skip_all, fields(peer_id = %self.swarm.local_peer_id()))]
    pub async fn run(mut self) {
        // Kick it off
        loop {
            tokio::select! {
                cmd = self.cmd_receiver.recv() => {
                    // If no listeners then we exit
                    let Some(cmd) = cmd else { return; };
                    self.handle_command(cmd)
                }
                event = self.swarm.select_next_some() => match event {
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer_id, multiaddr) in list {
                            tracing::debug!(?peer_id, %multiaddr, "mDNS discovered a new peer");
                            self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                        }
                    },
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                        for (peer_id, multiaddr) in list {
                            tracing::debug!(?peer_id, %multiaddr, "mDNS discover peer has expired");
                            self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        }
                    },
                    // Send auth as new peer is subscribed
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed {
                        topic,
                        ..
                    })) if topic == *topics::Auth::hash() => {
                        let auth = bincode::serialize(&Auth {
                            peer_id: *self.swarm.local_peer_id(),
                            claim: self.jwt.claim().expect("Encoding JWT never fails"),
                        })
                        .expect("bincode ser never fails");

                        let result =
                            self.swarm
                            .behaviour_mut()
                            .gossipsub
                            .publish(topics::Auth::topic().clone(), auth);
                        if let Err(err) = result {
                            tracing::debug!(?err, "Failed to send auth message because of new peer");
                        }
                    }
                    SwarmEvent::Behaviour(LumioBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        message,
                        ..
                    })) => self.handle_message(message).await,
                    SwarmEvent::NewListenAddr { address, .. } => tracing::debug!("Local node is listening on {address}"),
                    event => tracing::trace!(?event),
                }
            }
        }
    }

    pub fn authorize(&mut self, source: PeerId, claim: String) -> Result<()> {
        self.jwt
            .decode(claim)
            .context("Failed to decode JWT claim")?;
        self.authorized.insert(source);
        Ok(())
    }
}
