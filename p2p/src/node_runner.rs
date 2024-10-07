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
    topics, Auth, Command, JwtSecret, LumioBehaviour, LumioBehaviourEvent, LumioCommand,
    SubscribeCommand,
};

trait IsInterested<Msg> {
    fn is_interested(&self, msg: &gossipsub::Message) -> bool;
}

impl<A, T: Topic + Default> IsInterested<T> for A {
    fn is_interested(&self, msg: &gossipsub::Message) -> bool {
        msg.topic == T::default().hash()
    }
}

trait HandleMsg<T: Topic> {
    fn handle_msg(
        &mut self,
        msg: T::Msg,
        extra: &gossipsub::Message,
    ) -> impl Future<Output = Result<()>>;
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
    lumio_move_events_since_subs: HashMap<TopicHash, tokio::sync::mpsc::Sender<SlotAttribute>>,
    lumio_sol_events_since_subs: HashMap<TopicHash, tokio::sync::mpsc::Sender<SlotAttribute>>,

    op_move_events_since_handler:
        Option<tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>)>>,
    op_sol_events_since_handler:
        Option<tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>)>>,
    lumio_move_events_since_handler:
        Option<tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotAttribute>)>>,
    lumio_sol_events_since_handler:
        Option<tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotAttribute>)>>,
}

impl HandleMsg<topics::Auth> for NodeRunner {
    async fn handle_msg(
        &mut self,
        Auth { peer_id, claim }: Auth,
        _: &gossipsub::Message,
    ) -> Result<()> {
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
            async fn handle_msg(
                &mut self,
                msg: <$topic as topics::Topic>::Msg,
                _: &gossipsub::Message,
            ) -> Result<()> {
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

impl HandleMsg<topics::OpMoveCommands> for NodeRunner {
    async fn handle_msg(
        &mut self,
        topic: topics::OpMoveEventsSince,
        _: &gossipsub::Message,
    ) -> Result<()> {
        let ch = self
            .op_move_events_since_handler
            .as_ref()
            .expect("We should always have a channel if we subscribed to topic");
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
        Ok(())
    }
}

impl HandleMsg<topics::OpSolCommands> for NodeRunner {
    async fn handle_msg(
        &mut self,
        topic: topics::OpSolEventsSince,
        _: &gossipsub::Message,
    ) -> Result<()> {
        let ch = self
            .op_sol_events_since_handler
            .as_ref()
            .expect("We should always have a channel if we subscribed to topic");
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
        Ok(())
    }
}

impl HandleMsg<topics::LumioCommands> for NodeRunner {
    async fn handle_msg(&mut self, cmd: LumioCommand, _: &gossipsub::Message) -> Result<()> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let topic = match cmd {
            LumioCommand::SolSubscribeSince(topic) => {
                let ch = self
                    .lumio_sol_events_since_handler
                    .as_ref()
                    .expect("We should always have a channel if we subscribed to topic");
                let _ = ch.send((topic.0, sender)).await;
                topic.topic()
            }
            LumioCommand::MoveSubscribeSince(topic) => {
                let ch = self
                    .lumio_move_events_since_handler
                    .as_ref()
                    .expect("We should always have a channel if we subscribed to topic");
                let _ = ch.send((topic.0, sender)).await;
                topic.topic()
            }
        };
        tokio::spawn({
            let cmd_sender = self.cmd_sender.clone();
            async move {
                while let Some(msg) = receiver.recv().await {
                    let msg = bincode::serialize(&msg).unwrap();
                    if cmd_sender
                        .send(Command::SendEvent(topic.clone(), msg))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        });
        Ok(())
    }
}

impl IsInterested<topics::LumioMoveEventsSince> for NodeRunner {
    fn is_interested(&self, msg: &gossipsub::Message) -> bool {
        self.lumio_move_events_since_subs.contains_key(&msg.topic)
    }
}

impl HandleMsg<topics::LumioMoveEventsSince> for NodeRunner {
    async fn handle_msg(
        &mut self,
        msg: SlotAttribute,
        gossipsub::Message { topic, .. }: &gossipsub::Message,
    ) -> Result<()> {
        let _ = self
            .lumio_move_events_since_subs
            .get(topic)
            .unwrap()
            .send(msg)
            .await;
        Ok(())
    }
}

impl IsInterested<topics::LumioSolEventsSince> for NodeRunner {
    fn is_interested(&self, msg: &gossipsub::Message) -> bool {
        self.lumio_sol_events_since_subs.contains_key(&msg.topic)
    }
}

impl HandleMsg<topics::LumioSolEventsSince> for NodeRunner {
    async fn handle_msg(
        &mut self,
        msg: SlotAttribute,
        gossipsub::Message { topic, .. }: &gossipsub::Message,
    ) -> Result<()> {
        let _ = self
            .lumio_sol_events_since_subs
            .get(topic)
            .unwrap()
            .send(msg)
            .await;
        Ok(())
    }
}

impl IsInterested<topics::OpMoveEventsSince> for NodeRunner {
    fn is_interested(&self, msg: &gossipsub::Message) -> bool {
        self.op_move_events_since_subs.contains_key(&msg.topic)
    }
}

impl HandleMsg<topics::OpMoveEventsSince> for NodeRunner {
    async fn handle_msg(
        &mut self,
        msg: SlotPayloadWithEvents,
        gossipsub::Message { topic, .. }: &gossipsub::Message,
    ) -> Result<()> {
        let _ = self
            .op_move_events_since_subs
            .get(topic)
            .unwrap()
            .send(msg)
            .await;
        Ok(())
    }
}

impl IsInterested<topics::OpSolEventsSince> for NodeRunner {
    fn is_interested(&self, msg: &gossipsub::Message) -> bool {
        self.op_sol_events_since_subs.contains_key(&msg.topic)
    }
}

impl HandleMsg<topics::OpSolEventsSince> for NodeRunner {
    async fn handle_msg(
        &mut self,
        msg: SlotPayloadWithEvents,
        gossipsub::Message { topic, .. }: &gossipsub::Message,
    ) -> Result<()> {
        let _ = self
            .op_sol_events_since_subs
            .get(topic)
            .unwrap()
            .send(msg)
            .await;
        Ok(())
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
            lumio_move_events_since_handler: None,
            lumio_sol_events_since_handler: None,
            lumio_move_events_since_subs: Default::default(),
            lumio_sol_events_since_subs: Default::default(),
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
            SubscribeCommand::LumioOpSolSinceHandler(sender) => {
                self.lumio_sol_events_since_handler = Some(sender)
            }
            SubscribeCommand::LumioOpMoveSinceHandler(sender) => {
                self.lumio_move_events_since_handler = Some(sender)
            }
            SubscribeCommand::LumioOpMoveSince { sender, since } => {
                self.lumio_move_events_since_subs
                    .insert(topics::LumioMoveEventsSince(since).hash(), sender);
                self.publish_event(
                    topics::LumioCommands.hash(),
                    &LumioCommand::MoveSubscribeSince(topics::LumioMoveEventsSince(since)),
                )
            }
            SubscribeCommand::LumioOpSolSince { sender, since } => {
                self.lumio_sol_events_since_subs
                    .insert(topics::LumioSolEventsSince(since).hash(), sender);
                self.publish_event(
                    topics::LumioCommands.hash(),
                    &LumioCommand::SolSubscribeSince(topics::LumioSolEventsSince(since)),
                )
            }
        }
    }

    async fn try_run_msg<T>(&mut self, msg: &gossipsub::Message) -> ControlFlow<Result<()>>
    where
        T: Topic,
        Self: HandleMsg<T> + IsInterested<T>,
    {
        tracing::trace!(s = std::any::type_name::<T>(), "Handling");
        if !IsInterested::<T>::is_interested(self, msg) {
            return ControlFlow::Continue(());
        }
        let Ok(typed_msg) = bincode::deserialize::<T::Msg>(&msg.data) else {
            return ControlFlow::Break(Err(eyre::eyre!(
                "Failed to decode event for topic {}. Skipping...",
                std::any::type_name::<T>()
            )));
        };

        ControlFlow::Break(HandleMsg::<T>::handle_msg(self, typed_msg, msg).await)
    }

    #[must_use]
    async fn handle_topics(&mut self, msg: &gossipsub::Message) -> ControlFlow<Result<()>> {
        tracing::debug!("New message");
        self.try_run_msg::<topics::Auth>(msg).await?;

        let Some(source) = msg.source else {
            tracing::debug!(topic = ?msg.topic, "Ignoring message as sender is unknown");
            return ControlFlow::Break(Ok(()));
        };

        if !self.authorized.contains(&source) {
            tracing::trace!(?source, "Ignoring message from unauthorized peer");
            return ControlFlow::Break(Ok(()));
        }

        self.try_run_msg::<topics::OpMoveEvents>(msg).await?;
        self.try_run_msg::<topics::OpSolEvents>(msg).await?;
        self.try_run_msg::<topics::LumioMoveEvents>(msg).await?;
        self.try_run_msg::<topics::LumioSolEvents>(msg).await?;
        self.try_run_msg::<topics::OpMoveCommands>(msg).await?;
        self.try_run_msg::<topics::OpSolCommands>(msg).await?;
        self.try_run_msg::<topics::OpMoveEventsSince>(msg).await?;
        self.try_run_msg::<topics::OpSolEventsSince>(msg).await?;
        self.try_run_msg::<topics::LumioCommands>(msg).await?;
        self.try_run_msg::<topics::LumioMoveEventsSince>(msg)
            .await?;
        self.try_run_msg::<topics::LumioSolEventsSince>(msg).await?;

        tracing::debug!(topic = ?msg.topic, "Ignoring message from unknown topic");

        ControlFlow::Break(Ok(()))
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
                    })) => match self.handle_topics(&message).await {
                        ControlFlow::Break(Ok(())) => (),
                        ControlFlow::Break(Err(err)) => tracing::warn!(?err),
                        ControlFlow::Continue(()) => unreachable!("At this point we will always break"),
                    }
                    SwarmEvent::NewListenAddr { address, .. } => tracing::debug!("Local node is listening on {address}"),
                    event => tracing::trace!(?event),
                }
            }
        }
    }
}
