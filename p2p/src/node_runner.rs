use eyre::{Result, WrapErr};
use futures::prelude::*;
use libp2p::{
    gossipsub::{self, TopicHash},
    mdns,
    swarm::SwarmEvent,
    PeerId, Swarm,
};
use lumio_types::p2p::{SlotAttribute, SlotPayloadWithEvents};
use lumio_types::Slot;

use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;

use crate::topics::Topic;
use crate::{
    topics, Auth, Command, JwtSecret, LumioBehaviour, LumioBehaviourEvent, SubscribeCommand,
};

trait HandleMsg<T: Topic> {
    fn try_run_msg(
        &mut self,
        topic: T,
        msg: &gossipsub::Message,
    ) -> impl Future<Output = ControlFlow<Result<()>>> {
        async move {
            if msg.topic != Topic::hash(&topic) {
                return ControlFlow::Continue(());
            }
            let Ok(msg) = bincode::deserialize(&msg.data) else {
                return ControlFlow::Break(Err(eyre::eyre!(
                    "Failed to decode event for topic {}. Skipping...",
                    std::any::type_name::<T>()
                )));
            };

            ControlFlow::Break(self.handle_msg(msg).await)
        }
    }

    fn handle_msg(&mut self, msg: T::Msg) -> impl Future<Output = Result<()>>;
}

pub struct NodeRunner {
    swarm: Swarm<LumioBehaviour>,
    jwt: JwtSecret,
    cmd_receiver: tokio::sync::mpsc::Receiver<Command>,
    // For tasks
    cmd_sender: tokio::sync::mpsc::Sender<Command>,

    authorized: HashSet<PeerId>,

    op_move_events: Option<tokio::sync::mpsc::Sender<SlotPayloadWithEvents>>,
    op_sol_events: Option<tokio::sync::mpsc::Sender<SlotPayloadWithEvents>>,
    lumio_sol_events: Option<tokio::sync::mpsc::Sender<SlotAttribute>>,
    lumio_move_events: Option<tokio::sync::mpsc::Sender<SlotAttribute>>,

    op_move_events_since_subs: HashMap<TopicHash, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>>,
    op_sol_events_since_subs: HashMap<TopicHash, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>>,

    op_move_events_since_handler:
        Option<tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>)>>,
    op_sol_events_since_handler:
        Option<tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>)>>,
}

impl HandleMsg<topics::Auth> for NodeRunner {
    async fn handle_msg(&mut self, Auth { peer_id, claim }: Auth) -> Result<()> {
        self.jwt
            .decode(claim)
            .context("Failed to decode JWT claim")?;
        self.authorized.insert(peer_id);
        Ok(())
    }
}

macro_rules! impl_handle_channel {
    ($node_runner:ty {
        $($topic:ty => $field:ident),* $(,)?
    }) => {$(
        impl HandleMsg<$topic> for $node_runner {
            async fn handle_msg(&mut self, msg: <$topic as topics::Topic>::Msg) -> Result<()> {
                // TODO: unsub if noone listens
                let _ = self
                    . $field
                    .as_ref()
                    .expect("We should always have a channel if we subscribed to topic")
                    .send(msg)
                    .await;
                Ok(())
            }
        }
    )*}
}

impl_handle_channel! {
    NodeRunner {
        topics::OpMoveEvents => op_move_events,
        topics::OpSolEvents => op_sol_events,
        topics::LumioSolEvents => lumio_sol_events,
        topics::LumioMoveEvents => lumio_move_events,
    }
}

impl NodeRunner {
    pub(crate) fn new(
        swarm: Swarm<LumioBehaviour>,
        jwt: JwtSecret,
    ) -> (Self, tokio::sync::mpsc::Sender<Command>) {
        let (cmd_sender, cmd_receiver) = tokio::sync::mpsc::channel(100);
        let me = Self {
            swarm,
            jwt,
            cmd_receiver,
            cmd_sender: cmd_sender.clone(),

            authorized: Default::default(),
            op_move_events: None,
            op_sol_events: None,
            lumio_sol_events: None,
            lumio_move_events: None,
            op_move_events_since_subs: Default::default(),
            op_sol_events_since_subs: Default::default(),
            op_move_events_since_handler: None,
            op_sol_events_since_handler: None,
        };
        (me, cmd_sender)
    }

    fn publish(&mut self, hash: TopicHash, event: Vec<u8>) {
        match self
            .swarm
            .behaviour_mut()
            .gossipsub
            .publish(hash.clone(), event)
        {
            Ok(_) => (),
            Err(gossipsub::PublishError::InsufficientPeers) => {
                // TODO: print topic name
                tracing::warn!("Something might be wrong. Noone listens on topic hash {hash}")
            }
            Err(err) => panic!("Failed to publish message: {err}"),
        }
    }

    fn publish_event<E: serde::Serialize>(&mut self, hash: TopicHash, event: &E) {
        self.publish(
            hash,
            bincode::serialize(event).expect("bincode serialization never fails"),
        )
    }

    fn handle_command(&mut self, cmd: Command) {
        let cmd = match cmd {
            Command::Subscribe(cmd) => cmd,
            Command::SendEvent(topic, ev) => {
                self.publish(topic.hash(), ev);
                return;
            }
        };
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&cmd.topic())
            .expect("Something went terribly wrong, as we failed to subscribe to some topic");
        match cmd {
            SubscribeCommand::OpMove(sender) => self.op_move_events = Some(sender),
            SubscribeCommand::OpSol(sender) => self.op_sol_events = Some(sender),
            SubscribeCommand::LumioOpSol(sender) => self.lumio_sol_events = Some(sender),
            SubscribeCommand::LumioOpMove(sender) => self.lumio_move_events = Some(sender),
            SubscribeCommand::OpMoveSinceHandler(sender) => {
                self.op_move_events_since_handler = Some(sender)
            }
            SubscribeCommand::OpSolSinceHandler(sender) => {
                self.op_sol_events_since_handler = Some(sender)
            }
            SubscribeCommand::OpMoveSince { sender, since } => {
                self.op_move_events_since_subs
                    .insert(topics::OpMoveEventsSince(since).hash(), sender);
                self.publish_event(
                    topics::OpMoveCommands.hash(),
                    &topics::OpMoveEventsSince(since),
                )
            }
            SubscribeCommand::OpSolSince { sender, since } => {
                self.op_sol_events_since_subs
                    .insert(topics::OpSolEventsSince(since).hash(), sender);
                self.publish_event(
                    topics::OpSolCommands.hash(),
                    &topics::OpSolEventsSince(since),
                )
            }
        }
    }

    #[must_use]
    async fn handle_topics(&mut self, msg: &gossipsub::Message) -> ControlFlow<Result<()>> {
        self.try_run_msg(topics::Auth, msg).await?;

        let Some(source) = msg.source else {
            tracing::debug!(topic = ?msg.topic, "Ignoring message as sender is unknown");
            return ControlFlow::Break(Ok(()));
        };

        if !self.authorized.contains(&source) {
            tracing::trace!(?source, "Ignoring message from unauthorized peer");
            return ControlFlow::Break(Ok(()));
        }

        self.try_run_msg(topics::OpMoveEvents, msg).await?;
        self.try_run_msg(topics::OpSolEvents, msg).await?;
        self.try_run_msg(topics::LumioMoveEvents, msg).await?;
        self.try_run_msg(topics::LumioSolEvents, msg).await?;

        ControlFlow::Continue(())
    }

    async fn handle_message(&mut self, msg: gossipsub::Message) {
        match self.handle_topics(&msg).await {
            ControlFlow::Break(Ok(())) => return,
            ControlFlow::Break(Err(err)) => {
                tracing::warn!(?err);
                return;
            }
            ControlFlow::Continue(()) => (),
        }

        let gossipsub::Message { data, topic, .. } = msg;

        match topic {
            t if t == topics::OpMoveCommands.hash() => {
                let ch = self
                    .op_move_events_since_handler
                    .as_ref()
                    .expect("We should always have a channel if we subscribed to topic");
                let Ok(topic) = bincode::deserialize::<topics::OpMoveEventsSince>(&data) else {
                    tracing::debug!("Failed to decode op move commands. Skipping...");
                    return;
                };

                let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
                let _ = ch.send((topic.0, sender)).await;
                tokio::spawn({
                    let cmd_sender = self.cmd_sender.clone();
                    async move {
                        while let Some(msg) = receiver.recv().await {
                            let msg = bincode::serialize(&msg).unwrap();
                            if cmd_sender
                                .send(Command::SendEvent(topic.topic(), msg))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                });
            }
            t if t == topics::OpSolCommands.hash() => {
                let ch = self
                    .op_sol_events_since_handler
                    .as_ref()
                    .expect("We should always have a channel if we subscribed to topic");
                let Ok(topic) = bincode::deserialize::<topics::OpSolEventsSince>(&data) else {
                    tracing::debug!("Failed to decode op sol commands. Skipping...");
                    return;
                };

                let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
                let _ = ch.send((topic.0, sender)).await;
                tokio::spawn({
                    let cmd_sender = self.cmd_sender.clone();
                    async move {
                        while let Some(msg) = receiver.recv().await {
                            let msg = bincode::serialize(&msg).unwrap();
                            if cmd_sender
                                .send(Command::SendEvent(topic.topic(), msg))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                });
            }
            t if self.op_move_events_since_subs.contains_key(&t) => {
                let Ok(msg) = bincode::deserialize(&data) else {
                    tracing::debug!("Failed to decode op move events. Skipping...");
                    return;
                };
                let sender = self.op_move_events_since_subs.get(&t).unwrap();
                let _ = sender.send(msg).await;
            }
            t if self.op_sol_events_since_subs.contains_key(&t) => {
                let Ok(msg) = bincode::deserialize(&data) else {
                    tracing::debug!("Failed to decode op sol events. Skipping...");
                    return;
                };
                let sender = self.op_sol_events_since_subs.get(&t).unwrap();
                let _ = sender.send(msg).await;
            }
            topic => tracing::debug!(?topic, "Ignoring message from unknown topic"),
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
                    })) if topic == topics::Auth.hash() => {
                        let auth = bincode::serialize(&Auth {
                            peer_id: *self.swarm.local_peer_id(),
                            claim: self.jwt.claim().expect("Encoding JWT never fails"),
                        })
                            .expect("bincode ser never fails");

                        let result =
                            self.swarm
                            .behaviour_mut()
                            .gossipsub
                            .publish(topics::Auth.topic().clone(), auth);
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
}
