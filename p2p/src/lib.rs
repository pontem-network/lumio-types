use eyre::{Result, WrapErr};
use futures::prelude::*;
use libp2p::multiaddr::Multiaddr;
use libp2p::{gossipsub, mdns};
use lumio_types::rpc::{AttributesArtifact, LumioEvents};

use std::hash::{Hash, Hasher};
use std::time::Duration;

use node_runner::NodeRunner;

pub use jwt::JwtSecret;

mod jwt;
pub mod node_runner;

// We create a custom network behaviour that combines Gossipsub and Mdns.
#[derive(libp2p::swarm::NetworkBehaviour)]
struct LumioBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub listen_on: Vec<Multiaddr>,
    pub bootstrap_addresses: Vec<Multiaddr>,
    pub jwt: JwtSecret,
}

enum SubscribeCommand {
    /// Events from op-move to lumio
    OpMove(futures::channel::mpsc::Sender<AttributesArtifact>),
    /// Events from op-sol to lumio
    OpSol(futures::channel::mpsc::Sender<AttributesArtifact>),
    /// Events from lumio to op-sol
    LumioOpSol(futures::channel::mpsc::Sender<LumioEvents>),
    /// Events from lumio to op-move
    LumioOpMove(futures::channel::mpsc::Sender<LumioEvents>),
}

impl SubscribeCommand {
    pub fn topic(&self) -> &gossipsub::IdentTopic {
        match self {
            Self::OpMove(_) => &topics::OP_MOVE_EVENTS,
            Self::OpSol(_) => &topics::OP_SOL_EVENTS,
            Self::LumioOpSol(_) => &topics::LUMIO_SOL_EVENTS,
            Self::LumioOpMove(_) => &topics::LUMIO_MOVE_EVENTS,
        }
    }
}

enum SendEventCommand {
    /// Events from op-move to lumio
    OpMove(AttributesArtifact),
    /// Events from op-sol to lumio
    OpSol(AttributesArtifact),
    /// Events from lumio to op-sol
    LumioOpSol(LumioEvents),
    /// Events from lumio to op-move
    LumioOpMove(LumioEvents),
}

enum Command {
    Subscribe(SubscribeCommand),
    SendEvent(SendEventCommand),
}

#[derive(Debug, Clone)]
pub struct Node {
    cmd_sender: futures::channel::mpsc::Sender<Command>,
}

impl Node {
    pub fn new(cfg: Config) -> eyre::Result<(Self, NodeRunner)> {
        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
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
                let mut gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )?;

                gossipsub.subscribe(&topics::AUTH)?;

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
            swarm.dial(a).context("Failed to listen on address")?;
        }

        swarm
            .behaviour_mut()
            .gossipsub
            .publish(topics::AUTH.clone(), jwt.claim()?)
            .context("Failed to auth")?;
        let (cmd_sender, cmd_receiver) = futures::channel::mpsc::channel(100);

        Ok((
            Self { cmd_sender },
            NodeRunner::new(swarm, jwt, cmd_receiver),
        ))
    }

    pub async fn subscribe_op_move_events(
        &mut self,
    ) -> Result<impl Stream<Item = AttributesArtifact> + Unpin + 'static> {
        let (sender, receiver) = futures::channel::mpsc::channel(10);
        self.cmd_sender
            .send(Command::Subscribe(SubscribeCommand::OpMove(sender)))
            .await
            .context("Node runner is probably dead")?;
        Ok(receiver)
    }

    pub async fn subscribe_op_sol_events(
        &mut self,
    ) -> Result<impl Stream<Item = AttributesArtifact> + Unpin + 'static> {
        let (sender, receiver) = futures::channel::mpsc::channel(10);
        self.cmd_sender
            .send(Command::Subscribe(SubscribeCommand::OpSol(sender)))
            .await
            .context("Node runner is probably dead")?;
        Ok(receiver)
    }

    pub async fn subscribe_lumio_op_sol_events(
        &mut self,
    ) -> Result<impl Stream<Item = LumioEvents> + Unpin + 'static> {
        let (sender, receiver) = futures::channel::mpsc::channel(20);
        self.cmd_sender
            .send(Command::Subscribe(SubscribeCommand::LumioOpSol(sender)))
            .await
            .context("Node runner is probably dead")?;
        Ok(receiver)
    }

    pub async fn subscribe_lumio_op_move_events(
        &mut self,
    ) -> Result<impl Stream<Item = LumioEvents> + Unpin + 'static> {
        let (sender, receiver) = futures::channel::mpsc::channel(20);
        self.cmd_sender
            .send(Command::Subscribe(SubscribeCommand::LumioOpMove(sender)))
            .await
            .context("Node runner is probably dead")?;
        Ok(receiver)
    }

    pub async fn send_lumio_op_move(&mut self, events: LumioEvents) -> Result<()> {
        self.cmd_sender
            .send(Command::SendEvent(SendEventCommand::LumioOpMove(events)))
            .await
            .context("Node runner is probably dead")?;
        Ok(())
    }

    pub async fn send_lumio_op_sol(&mut self, events: LumioEvents) -> Result<()> {
        self.cmd_sender
            .send(Command::SendEvent(SendEventCommand::LumioOpSol(events)))
            .await
            .context("Node runner is probably dead")?;
        Ok(())
    }

    pub async fn send_op_sol(&mut self, artifacts: AttributesArtifact) -> Result<()> {
        self.cmd_sender
            .send(Command::SendEvent(SendEventCommand::OpSol(artifacts)))
            .await
            .context("Node runner is probably dead")?;
        Ok(())
    }

    pub async fn send_op_move(&mut self, artifacts: AttributesArtifact) -> Result<()> {
        self.cmd_sender
            .send(Command::SendEvent(SendEventCommand::OpMove(artifacts)))
            .await
            .context("Node runner is probably dead")?;
        Ok(())
    }
}

pub(crate) mod topics {
    use libp2p::gossipsub::IdentTopic;

    use std::sync::LazyLock;

    macro_rules! topic {
        ($(static $name:ident = $topic:literal ;)*) => {
            $(
                pub static $name: LazyLock<IdentTopic> = LazyLock::new(|| IdentTopic::new(concat!("/lumio/v1/", $topic)));
            )*
        }
    }

    topic! {
        static AUTH = "auth";
        static OP_MOVE_EVENTS = "op_move_events";
        static OP_SOL_EVENTS = "op_sol_events";
        static LUMIO_SOL_EVENTS = "lumio_sol_events";
        static LUMIO_MOVE_EVENTS = "lumio_move_events";
    }
}
