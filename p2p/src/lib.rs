#![cfg_attr(test, allow(unused_crate_dependencies))]

use eyre::{Result, WrapErr};
use futures::prelude::*;
use libp2p::gossipsub;
use libp2p::multiaddr::Multiaddr;
use lumio_types::events::l2::EngineActions;
use lumio_types::p2p::{SlotAttribute, SlotPayloadWithEvents};
use lumio_types::Slot;
use serde::{Deserialize, Serialize};

use std::hash::{Hash, Hasher};
use std::time::Duration;

use node_runner::NodeRunner;
use topics::Topic;

pub use jwt::JwtSecret;
pub use libp2p;

mod jwt;
pub mod node_runner;

// We create a custom network behaviour that combines Gossipsub and Mdns.
#[derive(libp2p::swarm::NetworkBehaviour)]
struct LumioBehaviour {
    gossipsub: gossipsub::Behaviour,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub listen_on: Vec<Multiaddr>,
    pub bootstrap_addresses: Vec<Multiaddr>,
    pub jwt: JwtSecret,
}

enum SubscribeCommand {
    /// Events from op-move to lumio
    Move(tokio::sync::mpsc::Sender<SlotPayloadWithEvents>),
    /// Events from op-move to lumio
    MoveSince {
        since: Slot,
        sender: tokio::sync::mpsc::Sender<SlotPayloadWithEvents>,
    },
    /// Events from op-move to op-sol
    MoveEngineSince {
        since: Slot,
        sender: tokio::sync::mpsc::Sender<EngineActions>,
    },
    /// Events from op-sol to lumio
    Sol(tokio::sync::mpsc::Sender<SlotPayloadWithEvents>),
    /// Events from op-sol to lumio
    SolSince {
        since: Slot,
        sender: tokio::sync::mpsc::Sender<SlotPayloadWithEvents>,
    },
    /// Events from op-sol to op-move
    SolEngineSince {
        since: Slot,
        sender: tokio::sync::mpsc::Sender<EngineActions>,
    },
    /// Events from lumio to op-sol
    LumioSol(tokio::sync::mpsc::Sender<SlotAttribute>),
    /// Events from lumio to op-sol
    LumioSolSince {
        since: Slot,
        sender: tokio::sync::mpsc::Sender<SlotAttribute>,
    },
    /// Events from lumio to op-move
    LumioMove(tokio::sync::mpsc::Sender<SlotAttribute>),
    /// Events from lumio to op-move
    LumioMoveSince {
        since: Slot,
        sender: tokio::sync::mpsc::Sender<SlotAttribute>,
    },
    /// Give away streams from `MoveSince` subscription
    MoveSinceHandler(
        tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>)>,
    ),
    /// Give away streams from `MoveEngineSince` subscription
    MoveEngineSinceHandler(
        tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<EngineActions>)>,
    ),
    /// Give away streams from `SolSince` subscription
    SolSinceHandler(
        tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>)>,
    ),
    /// Give away streams from `SolEngineSince` subscription
    SolEngineSinceHandler(
        tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<EngineActions>)>,
    ),
    /// Give away streams from `LumioSolSince` subscription
    LumioSolSinceHandler(
        tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotAttribute>)>,
    ),
    /// Give away streams from `LumioMoveSince` subscription
    LumioMoveSinceHandler(
        tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotAttribute>)>,
    ),
}

impl SubscribeCommand {
    pub fn topic(&self) -> gossipsub::IdentTopic {
        match self {
            Self::Move(_) => topics::MoveEvents.topic(),
            Self::MoveSince { since, .. } => topics::MoveEventsSince(*since).topic(),
            Self::MoveEngineSince { since, .. } => topics::MoveEngineSince(*since).topic(),
            Self::Sol(_) => topics::SolEvents.topic(),
            Self::SolSince { since, .. } => topics::SolEventsSince(*since).topic(),
            Self::SolEngineSince { since, .. } => topics::SolEngineSince(*since).topic(),
            Self::LumioSolSince { since, .. } => topics::LumioSolEventsSince(*since).topic(),
            Self::LumioMoveSince { since, .. } => topics::LumioMoveEventsSince(*since).topic(),
            Self::LumioSol(_) => topics::LumioSolEvents.topic(),
            Self::LumioMove(_) => topics::LumioMoveEvents.topic(),
            Self::MoveSinceHandler(_) | Self::MoveEngineSinceHandler(_) => {
                topics::MoveCommands.topic()
            }
            Self::SolSinceHandler(_) | Self::SolEngineSinceHandler(_) => {
                topics::SolCommands.topic()
            }
            Self::LumioSolSinceHandler(_) | Self::LumioMoveSinceHandler(_) => {
                topics::LumioCommands.topic()
            }
        }
    }
}

enum Command {
    Subscribe(SubscribeCommand),
    SendEvent(gossipsub::IdentTopic, Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct Node {
    cmd_sender: tokio::sync::mpsc::Sender<Command>,
}

impl Node {
    pub fn new(
        keypair: libp2p::identity::Keypair,
        cfg: Config,
    ) -> eyre::Result<(Self, NodeRunner)> {
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
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
                    message.hash(&mut s);
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

                Ok(LumioBehaviour { gossipsub })
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
            swarm.dial(a).context("Failed to dial address")?;
        }

        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&topics::Auth.topic())?;

        match swarm
            .behaviour_mut()
            .gossipsub
            .publish(topics::Auth.topic().clone(), jwt.claim()?)
        {
            Ok(_) => tracing::debug!("Send auth message"),
            // We don't care if there no peers at this stage. We'll auth once someone subscribes
            Err(gossipsub::PublishError::InsufficientPeers) => tracing::warn!("No peers for auth"),
            Err(err) => return Err(err).context("Failed to auth"),
        }
        let (node_runner, cmd_sender) = NodeRunner::new(swarm, jwt);
        Ok((Self { cmd_sender }, node_runner))
    }

    async fn subscribe_event(&self, cmd: SubscribeCommand) -> Result<()> {
        self.cmd_sender
            .send(Command::Subscribe(cmd))
            .await
            .context("Node runner is probably dead")
    }

    pub async fn handle_op_move_since(
        &self,
    ) -> Result<
        impl Stream<Item = (Slot, impl Sink<SlotPayloadWithEvents> + Unpin + 'static)> + Unpin + 'static,
    > {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::MoveSinceHandler(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver)
            .map(|(slot, sender)| (slot, tokio_util::sync::PollSender::new(sender))))
    }

    pub async fn handle_op_move_engine_since(
        &self,
    ) -> Result<
        impl Stream<Item = (Slot, impl Sink<EngineActions> + Unpin + 'static)> + Unpin + 'static,
    > {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::MoveEngineSinceHandler(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver)
            .map(|(slot, sender)| (slot, tokio_util::sync::PollSender::new(sender))))
    }

    pub async fn handle_op_sol_since(
        &self,
    ) -> Result<
        impl Stream<Item = (Slot, impl Sink<SlotPayloadWithEvents> + Unpin + 'static)> + Unpin + 'static,
    > {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::SolSinceHandler(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver)
            .map(|(slot, sender)| (slot, tokio_util::sync::PollSender::new(sender))))
    }

    pub async fn handle_op_sol_engine_since(
        &self,
    ) -> Result<
        impl Stream<Item = (Slot, impl Sink<EngineActions> + Unpin + 'static)> + Unpin + 'static,
    > {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::SolEngineSinceHandler(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver)
            .map(|(slot, sender)| (slot, tokio_util::sync::PollSender::new(sender))))
    }

    pub async fn handle_lumio_move_since(
        &self,
    ) -> Result<
        impl Stream<Item = (Slot, impl Sink<SlotAttribute> + Unpin + 'static)> + Unpin + 'static,
    > {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::LumioMoveSinceHandler(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver)
            .map(|(slot, sender)| (slot, tokio_util::sync::PollSender::new(sender))))
    }

    pub async fn handle_lumio_sol_since(
        &self,
    ) -> Result<
        impl Stream<Item = (Slot, impl Sink<SlotAttribute> + Unpin + 'static)> + Unpin + 'static,
    > {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::LumioSolSinceHandler(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver)
            .map(|(slot, sender)| (slot, tokio_util::sync::PollSender::new(sender))))
    }

    pub async fn subscribe_op_move_events(
        &self,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::Move(sender)).await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_op_sol_events(
        &self,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::Sol(sender)).await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_op_move_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::MoveSince { since, sender })
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_op_sol_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::SolSince { since, sender })
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_lumio_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::MoveSince { since, sender })
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_lumio_op_sol_events(
        &self,
    ) -> Result<impl Stream<Item = SlotAttribute> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(20);
        self.subscribe_event(SubscribeCommand::LumioSol(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_lumio_op_sol_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotAttribute> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::LumioSolSince { since, sender })
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_lumio_op_move_events(
        &self,
    ) -> Result<impl Stream<Item = SlotAttribute> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(20);
        self.subscribe_event(SubscribeCommand::LumioMove(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_lumio_op_move_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotAttribute> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::LumioMoveSince { since, sender })
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_lumio_op_move_engine_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = EngineActions> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::MoveEngineSince { since, sender })
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_lumio_op_sol_engine_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = EngineActions> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::SolEngineSince { since, sender })
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    async fn send_event(&self, topic: impl Topic, event: impl serde::Serialize) -> Result<()> {
        tracing::debug!(topic = %topic.topic(), "Sending event");
        self.cmd_sender
            .send(Command::SendEvent(
                topic.topic(),
                bincode::serialize(&event).unwrap(),
            ))
            .await
            .context("Node runner is probably dead")?;
        Ok(())
    }

    pub async fn send_lumio_op_move(&self, events: SlotAttribute) -> Result<()> {
        self.send_event(topics::LumioMoveEvents, events).await
    }

    pub async fn send_lumio_op_sol(&self, events: SlotAttribute) -> Result<()> {
        self.send_event(topics::LumioSolEvents, events).await
    }

    pub async fn send_op_sol(&self, artifacts: SlotPayloadWithEvents) -> Result<()> {
        self.send_event(topics::SolEvents, artifacts).await
    }

    pub async fn send_op_move(&self, artifacts: SlotPayloadWithEvents) -> Result<()> {
        self.send_event(topics::MoveEvents, artifacts).await
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Auth {
    peer_id: libp2p::PeerId,
    claim: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) enum LumioCommand {
    SolSubscribeSince(topics::LumioSolEventsSince),
    MoveSubscribeSince(topics::LumioMoveEventsSince),
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) enum SolCommand {
    SubEventsSince(topics::SolEventsSince),
    EngineSince(topics::SolEngineSince),
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) enum MoveCommand {
    SubEventsSince(topics::MoveEventsSince),
    EngineSince(topics::MoveEngineSince),
}

pub(crate) mod topics {
    use libp2p::gossipsub::{IdentTopic, TopicHash};
    use lumio_types::events::l2::EngineActions;
    use lumio_types::p2p::{SlotAttribute, SlotPayloadWithEvents};
    use serde::{Deserialize, Serialize};

    use std::sync::LazyLock;

    pub(crate) trait Topic {
        type Msg: serde::Serialize + serde::de::DeserializeOwned;

        fn topic(&self) -> IdentTopic;

        fn hash(&self) -> TopicHash {
            self.topic().hash()
        }
    }

    macro_rules! topic {
        ( $(struct $name:ident <$msg:ty> ($topic:literal) ;)* ) => {$(
            #[derive(Clone, Copy, Debug, Default)]
            pub(crate) struct $name;

            impl Topic for $name {
                type Msg = $msg;
                fn topic(&self) -> IdentTopic {
                    static TOPIC: LazyLock<IdentTopic> = LazyLock::new(|| IdentTopic::new(concat!("/lumio/v1/", $topic)));
                    TOPIC.clone()
                }
                fn hash(&self) -> TopicHash {
                    static HASH: LazyLock<TopicHash> = LazyLock::new(|| $name.topic().hash());
                    HASH.clone()
                }
            }
        )*}
    }

    topic! {
        struct Auth<crate::Auth>("auth");
        struct MoveEvents<SlotPayloadWithEvents>("move/events");
        struct SolEvents<SlotPayloadWithEvents>("sol/events");
        struct LumioSolEvents<SlotAttribute>("lumio/sol/events");
        struct LumioMoveEvents<SlotAttribute>("lumio/move/events");

        struct LumioCommands<crate::LumioCommand>("lumio/cmds");
        struct SolCommands<crate::SolCommand>("sol/cmds");
        struct MoveCommands<crate::MoveCommand>("move/cmds");
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct SolEventsSince(pub lumio_types::Slot);

    impl Topic for SolEventsSince {
        type Msg = SlotPayloadWithEvents;
        fn topic(&self) -> IdentTopic {
            IdentTopic::new(format!("/lumio/v1/sol/events/since/{}", self.0))
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct MoveEventsSince(pub lumio_types::Slot);

    impl Topic for MoveEventsSince {
        type Msg = SlotPayloadWithEvents;
        fn topic(&self) -> IdentTopic {
            IdentTopic::new(format!("/lumio/v1/move/events/since/{}", self.0))
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct LumioSolEventsSince(pub lumio_types::Slot);

    impl Topic for LumioSolEventsSince {
        type Msg = SlotAttribute;
        fn topic(&self) -> IdentTopic {
            IdentTopic::new(format!("/lumio/v1/sol/events/since/{}", self.0))
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct LumioMoveEventsSince(pub lumio_types::Slot);

    impl Topic for LumioMoveEventsSince {
        type Msg = SlotAttribute;
        fn topic(&self) -> IdentTopic {
            IdentTopic::new(format!("/lumio/v1/lumio/move/events/since/{}", self.0))
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct MoveEngineSince(pub lumio_types::Slot);

    impl Topic for MoveEngineSince {
        type Msg = EngineActions;
        fn topic(&self) -> IdentTopic {
            IdentTopic::new(format!("/lumio/v1/move/engine/since/{}", self.0))
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct SolEngineSince(pub lumio_types::Slot);

    impl Topic for SolEngineSince {
        type Msg = EngineActions;
        fn topic(&self) -> IdentTopic {
            IdentTopic::new(format!("/lumio/v1/sol/engine/since/{}", self.0))
        }
    }
}
