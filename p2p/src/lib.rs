#![cfg_attr(test, allow(unused_crate_dependencies))]

use eyre::{Result, WrapErr};
use futures::prelude::*;
use libp2p::multiaddr::Multiaddr;
use libp2p::{gossipsub, mdns};
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
    mdns: mdns::tokio::Behaviour,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub listen_on: Vec<Multiaddr>,
    pub bootstrap_addresses: Vec<Multiaddr>,
    pub jwt: JwtSecret,
}

enum SubscribeCommand {
    /// Events from op-move to lumio
    OpMove(tokio::sync::mpsc::Sender<SlotPayloadWithEvents>),
    /// Events from op-move to lumio
    OpMoveSince {
        since: Slot,
        sender: tokio::sync::mpsc::Sender<SlotPayloadWithEvents>,
    },
    /// Events from op-sol to lumio
    OpSol(tokio::sync::mpsc::Sender<SlotPayloadWithEvents>),
    /// Events from op-sol to lumio
    OpSolSince {
        since: Slot,
        sender: tokio::sync::mpsc::Sender<SlotPayloadWithEvents>,
    },
    /// Events from lumio to op-sol
    LumioOpSol(tokio::sync::mpsc::Sender<SlotAttribute>),
    /// Events from lumio to op-sol
    LumioOpSolSince {
        since: Slot,
        sender: tokio::sync::mpsc::Sender<SlotAttribute>,
    },
    /// Events from lumio to op-move
    LumioOpMove(tokio::sync::mpsc::Sender<SlotAttribute>),
    /// Events from lumio to op-move
    LumioOpMoveSince {
        since: Slot,
        sender: tokio::sync::mpsc::Sender<SlotAttribute>,
    },
    /// Give away streams from `OpMoveSince` subscription
    OpMoveSinceHandler(
        tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>)>,
    ),
    /// Give away streams from `OpSolSince` subscription
    OpSolSinceHandler(
        tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotPayloadWithEvents>)>,
    ),
    /// Give away streams from `LumioOpSolSince` subscription
    LumioOpSolSinceHandler(
        tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotAttribute>)>,
    ),
    /// Give away streams from `LumioOpMoveSince` subscription
    LumioOpMoveSinceHandler(
        tokio::sync::mpsc::Sender<(Slot, tokio::sync::mpsc::Sender<SlotAttribute>)>,
    ),
}

impl SubscribeCommand {
    pub fn topic(&self) -> gossipsub::IdentTopic {
        match self {
            Self::OpMove(_) => topics::OpMoveEvents.topic(),
            Self::OpMoveSince { since, .. } => topics::OpMoveEventsSince(*since).topic(),
            Self::OpSol(_) => topics::OpSolEvents.topic(),
            Self::OpSolSince { since, .. } => topics::OpSolEventsSince(*since).topic(),
            Self::LumioOpSolSince { since, .. } => topics::LumioSolEventsSince(*since).topic(),
            Self::LumioOpMoveSince { since, .. } => topics::LumioMoveEventsSince(*since).topic(),
            Self::LumioOpSol(_) => topics::LumioSolEvents.topic(),
            Self::LumioOpMove(_) => topics::LumioMoveEvents.topic(),
            Self::OpMoveSinceHandler(_) => topics::OpMoveCommands.topic(),
            Self::OpSolSinceHandler(_) => topics::OpSolCommands.topic(),
            Self::LumioOpSolSinceHandler(_) | Self::LumioOpMoveSinceHandler(_) => {
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
        self.subscribe_event(SubscribeCommand::OpMoveSinceHandler(sender))
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
        self.subscribe_event(SubscribeCommand::OpSolSinceHandler(sender))
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
        self.subscribe_event(SubscribeCommand::LumioOpMoveSinceHandler(sender))
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
        self.subscribe_event(SubscribeCommand::LumioOpSolSinceHandler(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver)
            .map(|(slot, sender)| (slot, tokio_util::sync::PollSender::new(sender))))
    }

    pub async fn subscribe_op_move_events(
        &self,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::OpMove(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_op_sol_events(
        &self,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::OpSol(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_op_move_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::OpMoveSince { since, sender })
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_op_sol_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::OpSolSince { since, sender })
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_lumio_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotPayloadWithEvents> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::OpMoveSince { since, sender })
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_lumio_op_sol_events(
        &self,
    ) -> Result<impl Stream<Item = SlotAttribute> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(20);
        self.subscribe_event(SubscribeCommand::LumioOpSol(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_lumio_op_sol_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotAttribute> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::LumioOpSolSince { since, sender })
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_lumio_op_move_events(
        &self,
    ) -> Result<impl Stream<Item = SlotAttribute> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(20);
        self.subscribe_event(SubscribeCommand::LumioOpMove(sender))
            .await?;
        Ok(tokio_stream::wrappers::ReceiverStream::new(receiver))
    }

    pub async fn subscribe_lumio_op_move_events_since(
        &self,
        since: Slot,
    ) -> Result<impl Stream<Item = SlotAttribute> + Unpin + 'static> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscribe_event(SubscribeCommand::LumioOpMoveSince { since, sender })
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
        self.send_event(topics::OpSolEvents, artifacts).await
    }

    pub async fn send_op_move(&self, artifacts: SlotPayloadWithEvents) -> Result<()> {
        self.send_event(topics::OpMoveEvents, artifacts).await
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

pub(crate) mod topics {
    use libp2p::gossipsub::{IdentTopic, TopicHash};
    use lumio_types::p2p::{SlotAttribute, SlotPayloadWithEvents};
    use serde::{Deserialize, Serialize};

    use std::sync::LazyLock;

    pub(crate) trait Topic {
        type Msg: serde::Serialize + serde::de::DeserializeOwned;

        fn topic(&self) -> IdentTopic;

        fn hash(&self) -> TopicHash;
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
        struct OpMoveEvents<SlotPayloadWithEvents>("op_move_events");
        struct OpSolEvents<SlotPayloadWithEvents>("op_sol_events");
        struct LumioSolEvents<SlotAttribute>("lumio_sol_events");
        struct LumioMoveEvents<SlotAttribute>("lumio_move_events");

        struct LumioCommands<crate::LumioCommand>("lumio_cmds");
        struct OpSolCommands<OpSolEventsSince>("op_sol_cmds");
        struct OpMoveCommands<OpMoveEventsSince>("op_move_cmds");
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct OpSolEventsSince(pub lumio_types::Slot);

    impl Topic for OpSolEventsSince {
        type Msg = SlotPayloadWithEvents;
        fn topic(&self) -> IdentTopic {
            IdentTopic::new(format!("/lumio/v1/op_sol_events/since/{}", self.0))
        }
        fn hash(&self) -> TopicHash {
            self.topic().hash()
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct OpMoveEventsSince(pub lumio_types::Slot);

    impl Topic for OpMoveEventsSince {
        type Msg = SlotPayloadWithEvents;
        fn topic(&self) -> IdentTopic {
            IdentTopic::new(format!("/lumio/v1/op_move_events/since/{}", self.0))
        }
        fn hash(&self) -> TopicHash {
            self.topic().hash()
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct LumioSolEventsSince(pub lumio_types::Slot);

    impl Topic for LumioSolEventsSince {
        type Msg = SlotAttribute;
        fn topic(&self) -> IdentTopic {
            IdentTopic::new(format!("/lumio/v1/lumio_sol_events/since/{}", self.0))
        }
        fn hash(&self) -> TopicHash {
            self.topic().hash()
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct LumioMoveEventsSince(pub lumio_types::Slot);

    impl Topic for LumioMoveEventsSince {
        type Msg = SlotAttribute;
        fn topic(&self) -> IdentTopic {
            IdentTopic::new(format!("/lumio/v1/lumio_move_events/since/{}", self.0))
        }
        fn hash(&self) -> TopicHash {
            self.topic().hash()
        }
    }
}
