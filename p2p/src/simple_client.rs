use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    time::Duration,
};

use eyre::{bail, Context, ContextCompat, Result};
use futures::prelude::*;
use libp2p::{
    gossipsub::{self, IdentTopic, Message as GossipMessage, TopicHash},
    pnet::PreSharedKey,
    swarm::SwarmEvent,
    PeerId, Swarm,
};
use tokio::{
    select,
    sync::mpsc::{self, Receiver, Sender},
    time::sleep,
};
use tracing::{debug, error, instrument, warn};

use crate::{create_swarm_lumio, libp2p::Multiaddr, LumioBehaviour, LumioBehaviourEvent};

pub struct P2PClient {
    _handler_message_service: tokio::task::JoinHandle<()>,

    input_event: Receiver<SwarmEvent<LumioBehaviourEvent>>,
    input_message: Receiver<GossipMessage>,
    swarm_sender: Sender<RequestSystem>,
}

impl P2PClient {
    #[instrument(level = "debug", skip(listen_on, bootstrap_addresses))]
    pub async fn new<'a, L1, L2, M1, M2>(
        listen_on: L1,
        bootstrap_addresses: L2,
        psk: PreSharedKey,
    ) -> eyre::Result<Self>
    where
        L1: IntoIterator<Item = M1>,
        L2: IntoIterator<Item = M2>,
        M1: TryInto<Multiaddr> + Debug,
        <M1 as TryInto<Multiaddr>>::Error: std::error::Error + Sync + Send + 'static,
        M2: TryInto<Multiaddr> + Debug,
        <M2 as TryInto<Multiaddr>>::Error: std::error::Error + Sync + Send + 'static,
    {
        let mut swarm = create_swarm_lumio(libp2p::identity::Keypair::generate_ed25519(), psk)?;

        for addr in listen_on.into_iter() {
            debug!(?addr, "listen_on");

            let addr = addr
                .try_into()
                .context("Failed to convert to `MultiAddr`")?;
            swarm
                .listen_on(addr)
                .context("Failed to listen on address")?;
        }

        let bootstrap_addresses = bootstrap_addresses
            .into_iter()
            .map(|addr| addr.try_into().context("Failed to convert to `MultiAddr`"))
            .collect::<Result<Vec<Multiaddr>>>()?;

        for addr in &bootstrap_addresses {
            debug!(?addr, "dial");

            swarm.dial(addr.clone()).context("Failed to dial address")?;
        }

        let (swarm_sender, mut swarm_receiver) = tokio::sync::mpsc::channel::<RequestSystem>(10);
        let (input_event_sender, input_event) = tokio::sync::mpsc::channel(10);
        let (input_message_sender, input_message) = tokio::sync::mpsc::channel::<GossipMessage>(10);

        let handler_message_service = tokio::spawn(async move {
            loop {
                select! {
                    // swarm
                    Some(request) = swarm_receiver.recv() => {
                        debug!(?request, "swarm");

                        if let Err(e) = Self::request_swarm(&mut swarm, request).await {
                            error!("swarm: {e:#?}");
                        }
                    }

                    // incoming events
                    event = swarm.select_next_some() => {
                        // System message processing
                        match &event {
                            SwarmEvent::NewListenAddr { address, .. } => tracing::debug!("Local node is listening on {address}"),

                            // Incoming message
                            SwarmEvent::Behaviour(LumioBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                                message,
                                ..
                            })) => {
                                debug!("Incoming message {}", message.topic);

                                if let Err(err) = input_message_sender.send(message.clone()).await {
                                    error!("{err:#?}");
                                }
                            }
                            // Other events
                            _ => {
                                if let Err(err) = input_event_sender.send(event).await {
                                    error!("{err:#?}");
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            _handler_message_service: handler_message_service,

            swarm_sender,
            input_event,
            input_message,
        })
    }

    async fn request_swarm(
        swarm: &mut Swarm<LumioBehaviour>,
        request: RequestSystem,
    ) -> Result<()> {
        match request {
            RequestSystem::PeersCount(callback) => {
                callback.send(swarm.connected_peers().count()).await?;
            }
            RequestSystem::CheckPeers { mut list, callback } => {
                for peer in swarm.connected_peers() {
                    list.remove(peer);
                }

                callback.send(list.is_empty()).await?;
            }
            RequestSystem::CheckSubscribes { mut list, callback } => {
                let mut not_found = false;
                for (conn_peer, sub_topics) in swarm.behaviour().gossipsub.all_peers() {
                    let Some(peer) = list.remove(conn_peer) else {
                        not_found = true;
                        break;
                    };

                    if !peer.iter().all(|topic| sub_topics.contains(&&topic.hash())) {
                        not_found = true;
                        break;
                    }
                }
                callback.send(list.is_empty() && !not_found).await?;
            }
            RequestSystem::Subscribe(topic) => {
                swarm
                    .behaviour_mut()
                    .gossipsub
                    .subscribe(&topic)
                    .context("Error when subscribing to a topic")?;
            }
            RequestSystem::Unsubscribe(topic) => {
                swarm
                    .behaviour_mut()
                    .gossipsub
                    .unsubscribe(&topic)
                    .context("Error when unsubscribing to a topic")?;
            }
            RequestSystem::Message(Message { topic, data }) => {
                swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(topic, data)
                    .context("Error when posting a message")?;
            }
        };
        Ok(())
    }

    #[instrument(level = "debug", skip_all)]
    pub async fn peers_count(&self) -> Result<usize> {
        let (callback, mut response) = mpsc::channel(1);
        self.swarm_sender
            .send(RequestSystem::PeersCount(callback))
            .await?;
        response
            .recv()
            .await
            .context("Failed to get the number of `peers`")
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn wait_peers_count(&self, need_count: usize) -> Result<usize> {
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        for n in 0..120 {
            match self.peers_count().await? {
                count if count >= need_count => return Ok(count),
                count => debug!("{n}#wait {need_count}:{count}"),
            };

            debug!("{n}#wait");
            interval.tick().await;
        }

        warn!("The waiting time has expired");

        Ok(0)
    }

    pub async fn peers_connected(&self, list: HashSet<PeerId>) -> Result<bool> {
        let (callback, mut response) = mpsc::channel(1);
        self.swarm_sender
            .send(RequestSystem::CheckPeers { list, callback })
            .await?;
        response
            .recv()
            .await
            .context("It was not possible to check whether the peers is connected")
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn wait_peers(&self, peer_ids: &[PeerId]) -> Result<()> {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        let list: HashSet<_> = peer_ids.iter().cloned().collect();

        for n in 0..120 {
            if self.peers_connected(list.clone()).await? {
                return Ok(());
            }

            debug!("{n}#wait");
            interval.tick().await;
        }
        bail!("The waiting time has expired")
    }

    #[instrument(level = "debug", skip(self))]
    pub async fn wait_subscribe_peers(&self, topic: IdentTopic, peer_ids: &[PeerId]) -> Result<()> {
        let (callback, mut response) = mpsc::channel(1);
        let request = RequestSystem::CheckSubscribes {
            list: peer_ids
                .iter()
                .cloned()
                .map(|peer| (peer, vec![topic.clone()]))
                .collect(),
            callback,
        };
        for n in 0..120 {
            self.swarm_sender
                .send(request.clone())
                .await
                .context("Check if the peers have subscribed to this topic")?;
            if response.recv().await.unwrap_or_default() {
                return Ok(());
            }

            debug!("{n}#wait");
            sleep(Duration::from_millis(200)).await;
        }
        bail!("The waiting time has expired")
    }

    pub async fn subscribe(&self, topic: IdentTopic) -> Result<()> {
        self.swarm_sender
            .send(RequestSystem::Subscribe(topic))
            .await
            .context("subscribe")
    }

    pub async fn unsubscribe(&self, topic: IdentTopic) -> Result<()> {
        self.swarm_sender
            .send(RequestSystem::Unsubscribe(topic))
            .await
            .context("subscribe")
    }

    pub async fn publish_bytes<T, M>(&self, topic: T, message: M) -> Result<()>
    where
        T: Into<TopicHash>,
        M: Into<Vec<u8>>,
    {
        self.swarm_sender
            .send(RequestSystem::Message(Message {
                topic: topic.into(),
                data: message.into(),
            }))
            .await
            .context("publish")
    }

    /// publication with serialization of the message
    pub async fn publish<T, M>(&self, topic: T, message: M) -> Result<()>
    where
        T: Into<TopicHash>,
        M: serde::ser::Serialize,
    {
        self.publish_bytes(
            topic,
            bincode::serialize(&message).context("Error when serializing the message")?,
        )
        .await
        .context("publication with serialization of the message")
    }

    pub async fn next_message(&mut self) -> Option<GossipMessage> {
        self.input_message.recv().await
    }

    pub async fn next_message_with_decode<T: serde::de::DeserializeOwned>(&mut self) -> Option<T> {
        while let Some(GossipMessage { data, .. }) = self.next_message().await {
            match bincode::deserialize(&data).context("Error decoding the message") {
                Ok(ev) => return Some(ev),
                Err(err) => warn!("{err:#?}"),
            };
        }
        None
    }

    pub async fn next_event(&mut self) -> Option<SwarmEvent<LumioBehaviourEvent>> {
        self.input_event.recv().await
    }
}

#[derive(Debug, Clone)]
struct Message {
    topic: TopicHash,
    data: Vec<u8>,
}

#[derive(Debug, Clone)]
enum RequestSystem {
    PeersCount(Sender<usize>),
    CheckPeers {
        list: HashSet<PeerId>,
        callback: Sender<bool>,
    },
    Subscribe(IdentTopic),
    Unsubscribe(IdentTopic),
    /// Check if the peers have subscribed to this topic
    CheckSubscribes {
        list: HashMap<PeerId, Vec<IdentTopic>>,
        callback: Sender<bool>,
    },
    Message(Message),
}
