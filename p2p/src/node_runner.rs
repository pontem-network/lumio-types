use eyre::Result;
use futures::prelude::*;
use libp2p::{
    gossipsub::{self, TopicHash},
    swarm::SwarmEvent,
    Swarm,
};
use lumio_types::events::l2::EngineActions;
use lumio_types::p2p::{SlotAttribute, SlotPayloadWithEvents};
use lumio_types::Slot;

use std::collections::HashMap;
use std::ops::ControlFlow;

use crate::topics::Topic;
use crate::{
    topics, Command, LumioBehaviour, LumioBehaviourEvent, LumioCommand, MoveCommand, SolCommand,
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
    cmd_receiver: tokio::sync::mpsc::Receiver<(Command, tokio::sync::oneshot::Sender<()>)>,
    // For tasks
    cmd_sender: tokio::sync::mpsc::Sender<(Command, tokio::sync::oneshot::Sender<()>)>,

    op_move_events: Option<tokio::sync::mpsc::Sender<SlotPayloadWithEvents>>,
    op_sol_events: Option<tokio::sync::mpsc::Sender<SlotPayloadWithEvents>>,
    lumio_sol_events: Option<tokio::sync::mpsc::Sender<SlotAttribute>>,
    lumio_move_events: Option<tokio::sync::mpsc::Sender<SlotAttribute>>,

    op_move_events_since_subs: HashMap<TopicHash, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>>,
    op_move_engine_since_subs: HashMap<TopicHash, tokio::sync::mpsc::Sender<EngineActions>>,
    op_sol_events_since_subs: HashMap<TopicHash, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>>,
    op_sol_engine_since_subs: HashMap<TopicHash, tokio::sync::mpsc::Sender<EngineActions>>,
    lumio_move_events_since_subs: HashMap<TopicHash, tokio::sync::mpsc::Sender<SlotAttribute>>,
    lumio_sol_events_since_subs: HashMap<TopicHash, tokio::sync::mpsc::Sender<SlotAttribute>>,

    op_move_events_since_handler:
        Option<tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>)>>,
    op_move_engine_since_handler:
        Option<tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<EngineActions>)>>,
    op_sol_events_since_handler:
        Option<tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>)>>,
    op_sol_engine_since_handler:
        Option<tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<EngineActions>)>>,
    lumio_move_events_since_handler:
        Option<tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotAttribute>)>>,
    lumio_sol_events_since_handler:
        Option<tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotAttribute>)>>,
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
        topics::MoveEvents => op_move_events,
        topics::SolEvents => op_sol_events,
        topics::LumioSolEvents => lumio_sol_events,
        topics::LumioMoveEvents => lumio_move_events,
    }
}

impl HandleMsg<topics::MoveCommands> for NodeRunner {
    async fn handle_msg(&mut self, cmd: MoveCommand, _: &gossipsub::Message) -> Result<()> {
        match cmd {
            MoveCommand::SubEventsSince(topic) => {
                let ch = self
                    .op_move_events_since_handler
                    .as_ref()
                    .expect("We should always have a channel if we subscribed to topic");
                let (sender, receiver) = tokio::sync::mpsc::channel(10);
                let _ = ch.send((topic.0, sender)).await;
                self.spawn_sending_messages_from_receiver(topic.topic(), receiver);
            }
            MoveCommand::EngineSince(topic) => {
                let ch = self
                    .op_move_engine_since_handler
                    .as_ref()
                    .expect("We should always have a channel if we subscribed to topic");
                let (sender, receiver) = tokio::sync::mpsc::channel(10);
                let _ = ch.send((topic.0, sender)).await;
                self.spawn_sending_messages_from_receiver(topic.topic(), receiver);
            }
        }

        Ok(())
    }
}

impl IsInterested<topics::MoveEngineSince> for NodeRunner {
    fn is_interested(&self, msg: &gossipsub::Message) -> bool {
        self.op_move_engine_since_subs.contains_key(&msg.topic)
    }
}

impl HandleMsg<topics::MoveEngineSince> for NodeRunner {
    async fn handle_msg(
        &mut self,
        msg: EngineActions,
        gossipsub::Message { topic, .. }: &gossipsub::Message,
    ) -> Result<()> {
        let _ = self
            .op_move_engine_since_subs
            .get(topic)
            .unwrap()
            .send(msg)
            .await;
        Ok(())
    }
}

impl HandleMsg<topics::SolCommands> for NodeRunner {
    async fn handle_msg(&mut self, cmd: SolCommand, _: &gossipsub::Message) -> Result<()> {
        match cmd {
            SolCommand::SubEventsSince(topic) => {
                let ch = self
                    .op_sol_events_since_handler
                    .as_ref()
                    .expect("We should always have a channel if we subscribed to topic");
                let (sender, receiver) = tokio::sync::mpsc::channel(10);
                let _ = ch.send((topic.0, sender)).await;
                self.spawn_sending_messages_from_receiver(topic.topic(), receiver);
            }
            SolCommand::EngineSince(topic) => {
                let ch = self
                    .op_sol_engine_since_handler
                    .as_ref()
                    .expect("We should always have a channel if we subscribed to topic");
                let (sender, receiver) = tokio::sync::mpsc::channel(10);
                let _ = ch.send((topic.0, sender)).await;
                self.spawn_sending_messages_from_receiver(topic.topic(), receiver);
            }
        }
        Ok(())
    }
}

impl IsInterested<topics::SolEngineSince> for NodeRunner {
    fn is_interested(&self, msg: &gossipsub::Message) -> bool {
        self.op_sol_engine_since_subs.contains_key(&msg.topic)
    }
}

impl HandleMsg<topics::SolEngineSince> for NodeRunner {
    async fn handle_msg(
        &mut self,
        msg: EngineActions,
        gossipsub::Message { topic, .. }: &gossipsub::Message,
    ) -> Result<()> {
        let _ = self
            .op_move_engine_since_subs
            .get(topic)
            .unwrap()
            .send(msg)
            .await;
        Ok(())
    }
}

impl HandleMsg<topics::LumioCommands> for NodeRunner {
    async fn handle_msg(&mut self, cmd: LumioCommand, _: &gossipsub::Message) -> Result<()> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
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

        self.spawn_sending_messages_from_receiver(topic, receiver);

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

impl IsInterested<topics::MoveEventsSince> for NodeRunner {
    fn is_interested(&self, msg: &gossipsub::Message) -> bool {
        self.op_move_events_since_subs.contains_key(&msg.topic)
    }
}

impl HandleMsg<topics::MoveEventsSince> for NodeRunner {
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

impl IsInterested<topics::SolEventsSince> for NodeRunner {
    fn is_interested(&self, msg: &gossipsub::Message) -> bool {
        self.op_sol_events_since_subs.contains_key(&msg.topic)
    }
}

impl HandleMsg<topics::SolEventsSince> for NodeRunner {
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
    fn spawn_sending_messages_from_receiver(
        &self,
        topic: gossipsub::IdentTopic,
        mut receiver: tokio::sync::mpsc::Receiver<impl serde::Serialize + Send + 'static>,
    ) {
        tokio::spawn({
            let cmd_sender = self.cmd_sender.clone();
            async move {
                while let Some(msg) = receiver.recv().await {
                    let msg = bincode::serialize(&msg).unwrap();
                    // TODO: probably receiver should be optional
                    let (sender, _) = tokio::sync::oneshot::channel();
                    if cmd_sender
                        .send((Command::SendEvent(topic.clone(), msg), sender))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        });
    }

    pub(crate) fn new(
        swarm: Swarm<LumioBehaviour>,
    ) -> (
        Self,
        tokio::sync::mpsc::Sender<(Command, tokio::sync::oneshot::Sender<()>)>,
    ) {
        let (cmd_sender, cmd_receiver) = tokio::sync::mpsc::channel(100);
        let me = Self {
            swarm,
            cmd_receiver,
            cmd_sender: cmd_sender.clone(),

            op_move_events: None,
            op_sol_events: None,
            lumio_sol_events: None,
            lumio_move_events: None,
            op_move_events_since_subs: Default::default(),
            op_move_engine_since_subs: Default::default(),
            op_sol_events_since_subs: Default::default(),
            op_sol_engine_since_subs: Default::default(),
            op_move_events_since_handler: None,
            op_move_engine_since_handler: None,
            op_sol_events_since_handler: None,
            op_sol_engine_since_handler: None,
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
            .publish(hash.clone(), event.clone())
        {
            Ok(_) => (),
            Err(gossipsub::PublishError::InsufficientPeers) => {
                // TODO: print topic name
                tracing::warn!("Something might be wrong. Noone listens on topic hash {hash}")
            }
            Err(err) => panic!("Failed to publish message: {err}"),
        }
    }

    fn publish_event<T>(&mut self, topic: &T, event: &T::Msg)
    where
        T: Topic,
        Self: HandleMsg<T> + IsInterested<T>,
    {
        tracing::trace!(topic = %topic.topic(), "Publishing event");

        let event = bincode::serialize(event).expect("bincode serialization never fails");
        match self
            .swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic.hash(), event)
        {
            Ok(_) => (),
            Err(gossipsub::PublishError::InsufficientPeers) => {
                // TODO: print topic name
                tracing::warn!(
                    "Something might be wrong. Noone listens on topic {}",
                    topic.topic(),
                );
            }
            Err(err) => panic!("Failed to publish message: {err}"),
        }
    }

    fn handle_command(&mut self, cmd: Command) {
        let cmd = match cmd {
            Command::Subscribe(cmd) => cmd,
            Command::SendEvent(topic, ev) => {
                self.publish(topic.hash(), ev);
                return;
            }
        };
        tracing::trace!(topic = %cmd.topic(), "Subscribing");
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&cmd.topic())
            .expect("Something went terribly wrong, as we failed to subscribe to some topic");
        match cmd {
            SubscribeCommand::Move(sender) => self.op_move_events = Some(sender),
            SubscribeCommand::Sol(sender) => self.op_sol_events = Some(sender),
            SubscribeCommand::LumioSol(sender) => self.lumio_sol_events = Some(sender),
            SubscribeCommand::LumioMove(sender) => self.lumio_move_events = Some(sender),
            SubscribeCommand::MoveSinceHandler(sender) => {
                self.op_move_events_since_handler = Some(sender)
            }
            SubscribeCommand::MoveEngineSinceHandler(sender) => {
                self.op_move_engine_since_handler = Some(sender)
            }
            SubscribeCommand::SolSinceHandler(sender) => {
                self.op_sol_events_since_handler = Some(sender)
            }
            SubscribeCommand::SolEngineSinceHandler(sender) => {
                self.op_sol_engine_since_handler = Some(sender)
            }
            SubscribeCommand::MoveSince { sender, since } => {
                self.op_move_events_since_subs
                    .insert(topics::MoveEventsSince(since).hash(), sender);
                self.publish_event(
                    &topics::MoveCommands,
                    &MoveCommand::SubEventsSince(topics::MoveEventsSince(since)),
                )
            }
            SubscribeCommand::MoveEngineSince { sender, since } => {
                self.op_move_engine_since_subs
                    .insert(topics::MoveEngineSince(since).hash(), sender);
                self.publish_event(
                    &topics::MoveCommands,
                    &MoveCommand::EngineSince(topics::MoveEngineSince(since)),
                )
            }
            SubscribeCommand::SolSince { sender, since } => {
                self.op_sol_events_since_subs
                    .insert(topics::SolEventsSince(since).hash(), sender);
                self.publish_event(
                    &topics::SolCommands,
                    &SolCommand::SubEventsSince(topics::SolEventsSince(since)),
                )
            }
            SubscribeCommand::SolEngineSince { sender, since } => {
                self.op_move_engine_since_subs
                    .insert(topics::SolEngineSince(since).hash(), sender);
                self.publish_event(
                    &topics::SolCommands,
                    &SolCommand::EngineSince(topics::SolEngineSince(since)),
                )
            }
            SubscribeCommand::LumioSolSinceHandler(sender) => {
                self.lumio_sol_events_since_handler = Some(sender)
            }
            SubscribeCommand::LumioMoveSinceHandler(sender) => {
                self.lumio_move_events_since_handler = Some(sender)
            }
            SubscribeCommand::LumioMoveSince { sender, since } => {
                self.lumio_move_events_since_subs
                    .insert(topics::LumioMoveEventsSince(since).hash(), sender);
                self.publish_event(
                    &topics::LumioCommands,
                    &LumioCommand::MoveSubscribeSince(topics::LumioMoveEventsSince(since)),
                )
            }
            SubscribeCommand::LumioSolSince { sender, since } => {
                self.lumio_sol_events_since_subs
                    .insert(topics::LumioSolEventsSince(since).hash(), sender);
                self.publish_event(
                    &topics::LumioCommands,
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
        if !IsInterested::<T>::is_interested(self, msg) {
            return ControlFlow::Continue(());
        }
        tracing::trace!(s = std::any::type_name::<T>(), "Handling");
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
        macro_rules! run_topics {
            ( $($topic:ident),* $(,)? ) => {$(
                self.try_run_msg::<topics::$topic>(msg).await?;
            )*}
        }

        run_topics! {
            LumioCommands,
            LumioMoveEvents,
            LumioMoveEventsSince,
            LumioSolEvents,
            LumioSolEventsSince,
            MoveCommands,
            MoveEngineSince,
            MoveEvents,
            MoveEventsSince,
            SolCommands,
            SolEngineSince,
            SolEvents,
            SolEventsSince,
        };

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
                    let Some((cmd, result)) = cmd else { return; };
                    self.handle_command(cmd);
                    let _ = result.send(());
                }
                event = self.swarm.select_next_some() => match event {
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
                },
            }
        }
    }
}
