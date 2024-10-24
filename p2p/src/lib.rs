#![cfg_attr(test, allow(unused_crate_dependencies))]

use eyre::{Result, WrapErr};
use futures::prelude::*;
use libp2p::identity::Keypair;
use libp2p::multiaddr::Multiaddr;
use libp2p::pnet::PreSharedKey;
use libp2p::{gossipsub, Swarm};
use lumio_types::events::l2::EngineActions;
use lumio_types::p2p::{PayloadStatus, SlotAttribute, SlotPayloadWithEvents};
use lumio_types::Slot;
use serde::{Deserialize, Serialize};

use std::hash::{Hash, Hasher};
use std::time::Duration;

use node_runner::NodeRunner;
use topics::Topic;

pub use libp2p;

pub mod node_runner;
pub mod simple_client;

// We create a custom network behaviour that combines Gossipsub and Mdns.
#[derive(libp2p::swarm::NetworkBehaviour)]
pub struct LumioBehaviour {
    gossipsub: gossipsub::Behaviour,
}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub listen_on: Vec<Multiaddr>,
    pub bootstrap_addresses: Vec<Multiaddr>,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub psk: libp2p::pnet::PreSharedKey,
}

enum SubscribeCommand {
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
    LumioSolSince {
        since: Slot,
        sender: tokio::sync::mpsc::Sender<SlotAttribute>,
    },
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
    /// Subscribe to finalization of payloads on move
    MoveFinalizeHandler(tokio::sync::mpsc::Sender<Finalize>),
    /// Give away streams from `SolSince` subscription
    SolSinceHandler(
        tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>)>,
    ),
    /// Give away streams from `SolEngineSince` subscription
    SolEngineSinceHandler(
        tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<EngineActions>)>,
    ),
    /// Subscribe to finalization of payloads on sol
    SolFinalizeHandler(tokio::sync::mpsc::Sender<Finalize>),
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
            Self::MoveSince { since, .. } => topics::MoveEventsSince(*since).topic(),
            Self::MoveEngineSince { since, .. } => topics::MoveEngineSince(*since).topic(),
            Self::SolSince { since, .. } => topics::SolEventsSince(*since).topic(),
            Self::SolEngineSince { since, .. } => topics::SolEngineSince(*since).topic(),
            Self::LumioSolSince { since, .. } => topics::LumioSolEventsSince(*since).topic(),
            Self::LumioMoveSince { since, .. } => topics::LumioMoveEventsSince(*since).topic(),
            Self::MoveFinalizeHandler(_) => topics::MoveFinalize.topic(),
            Self::SolFinalizeHandler(_) => topics::SolFinalize.topic(),
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
    cmd_sender: tokio::sync::mpsc::Sender<(Command, tokio::sync::oneshot::Sender<()>)>,
}

impl Node {
    pub fn new(
        keypair: libp2p::identity::Keypair,
        cfg: Config,
    ) -> eyre::Result<(Self, NodeRunner)> {
        let Config {
            listen_on,
            bootstrap_addresses,
            psk,
        } = cfg;

        let mut swarm = create_swarm_lumio(keypair, psk)?;

        for a in listen_on {
            swarm.listen_on(a).context("Failed to listen on address")?;
        }
        for a in bootstrap_addresses {
            swarm.dial(a).context("Failed to dial address")?;
        }

        let (node_runner, cmd_sender) = NodeRunner::new(swarm);
        Ok((Self { cmd_sender }, node_runner))
    }

    async fn send_cmd_and_wait_completion(&self, cmd: Command) -> Result<()> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.cmd_sender
            .send((cmd, sender))
            .await
            .context("Node runner is probably dead")?;
        if receiver.await.is_err() {
            return Err(eyre::eyre!("Node runner is probably dead"));
        }

        Ok(())
    }

    async fn subscribe_event(&self, cmd: SubscribeCommand) -> Result<()> {
        self.send_cmd_and_wait_completion(Command::Subscribe(cmd))
            .await
            .context("Failed to subscribe")
    }

    async fn send_event<T: Topic>(&self, topic: &T, event: &T::Msg) -> Result<()> {
        self.send_cmd_and_wait_completion(Command::SendEvent(
            topic.topic(),
            bincode::serialize(event).unwrap(),
        ))
        .await
        .context("Failed to send event")
    }

    pub async fn op_sol_finalize(&self, slot: Slot, status: PayloadStatus) -> Result<()> {
        self.send_event(&topics::SolFinalize, &Finalize { slot, status })
            .await
    }

    pub async fn op_move_finalize(&self, slot: Slot, status: PayloadStatus) -> Result<()> {
        self.send_event(&topics::MoveFinalize, &Finalize { slot, status })
            .await
    }

    pub async fn handle_op_sol_finalize(
        &self,
    ) -> Result<impl Stream<Item = (Slot, PayloadStatus)> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::SolFinalizeHandler(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver)
            .map(|Finalize { slot, status }| (slot, status)))
    }

    pub async fn handle_op_move_finalize(
        &self,
    ) -> Result<impl Stream<Item = (Slot, PayloadStatus)> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::MoveFinalizeHandler(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver)
            .map(|Finalize { slot, status }| (slot, status)))
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

    pub async fn subscribe_lumio_op_sol_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotAttribute> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::LumioSolSince { since, sender })
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
}

#[derive(Clone, Serialize, Deserialize)]
pub enum LumioCommand {
    SolSubscribeSince(topics::LumioSolEventsSince),
    MoveSubscribeSince(topics::LumioMoveEventsSince),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum SolCommand {
    SubEventsSince(topics::SolEventsSince),
    EngineSince(topics::SolEngineSince),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum MoveCommand {
    SubEventsSince(topics::MoveEventsSince),
    EngineSince(topics::MoveEngineSince),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Finalize {
    pub slot: Slot,
    pub status: lumio_types::p2p::PayloadStatus,
}

pub mod topics {
    use libp2p::gossipsub::{IdentTopic, TopicHash};
    use lumio_types::events::l2::EngineActions;
    use lumio_types::p2p::{SlotAttribute, SlotPayloadWithEvents};
    use serde::{Deserialize, Serialize};

    use std::sync::LazyLock;

    pub trait Topic {
        type Msg: serde::Serialize + serde::de::DeserializeOwned;

        fn topic(&self) -> IdentTopic;

        fn hash(&self) -> TopicHash {
            self.topic().hash()
        }
    }

    macro_rules! topic {
        ( $(struct $name:ident <$msg:ty> ($topic:literal) ;)* ) => {$(
            #[derive(Clone, Copy, Debug, Default)]
            pub struct $name;

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
        struct LumioCommands<crate::LumioCommand>("lumio/cmds");
        struct SolCommands<crate::SolCommand>("sol/cmds");
        struct MoveCommands<crate::MoveCommand>("move/cmds");

        struct SolFinalize<crate::Finalize>("sol/finalize");
        struct MoveFinalize<crate::Finalize>("move/finalize");
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

fn create_swarm_lumio(keypair: Keypair, psk: PreSharedKey) -> eyre::Result<Swarm<LumioBehaviour>> {
    let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_other_transport(|key| {
            use libp2p::Transport;

            libp2p::tcp::tokio::Transport::new(libp2p::tcp::Config::default().nodelay(true))
                .and_then(move |socket, _| libp2p::pnet::PnetConfig::new(psk).handshake(socket))
                .upgrade(libp2p::core::transport::upgrade::Version::V1Lazy)
                .authenticate(libp2p::noise::Config::new(key).expect("Failed to make noise config"))
                .multiplex(libp2p::yamux::Config::default())
        })?
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
    Ok(swarm)
}
