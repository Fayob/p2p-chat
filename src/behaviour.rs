use libp2p::{
    autonat, dcutr, gossipsub, identify, kad, ping, request_response, relay
};
use libp2p::kad::store::MemoryStore;
use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::swarm::{NetworkBehaviour};
use crate::message::{MessageRequest, MessageResponse};
use libp2p::mdns;
use libp2p::request_response::json;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ChatBehaviourEvent")]
pub struct ChatBehaviour {
    pub ping: ping::Behaviour,
    pub messaging: json::Behaviour<MessageRequest, MessageResponse>,
    pub mdns: Toggle<mdns::tokio::Behaviour>,
    pub identify: identify::Behaviour,
    pub kademlia: kad::Behaviour<MemoryStore>,
    pub autonat: autonat::Behaviour,
    pub relay_server: relay::Behaviour,
    pub relay_client: relay::client::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub gossipsub: gossipsub::Behaviour,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum ChatBehaviourEvent {
    Ping(ping::Event),
    Messaging(request_response::Event<MessageRequest, MessageResponse>),
    Mdns(mdns::Event),
    Identify(identify::Event),
    Kademlia(kad::Event),
    Autonat(autonat::Event),
    RelayServer(relay::Event),
    RelayClient(relay::client::Event),
    Dcutr(dcutr::Event),
    Gossipsub(gossipsub::Event),
}

impl From<ping::Event> for ChatBehaviourEvent {
    fn from(event: ping::Event) -> Self {
        ChatBehaviourEvent::Ping(event)
    }
}

impl From<request_response::Event<MessageRequest, MessageResponse>> for ChatBehaviourEvent {
    fn from(event: request_response::Event<MessageRequest, MessageResponse>) -> Self {
        ChatBehaviourEvent::Messaging(event)
    }
}

impl From<mdns::Event> for ChatBehaviourEvent {
    fn from(event: mdns::Event) -> Self {
        ChatBehaviourEvent::Mdns(event)
    }
}

impl From<identify::Event> for ChatBehaviourEvent {
    fn from(event: identify::Event) -> Self {
        ChatBehaviourEvent::Identify(event)
    }
}


impl From<autonat::Event> for ChatBehaviourEvent {
    fn from(event: autonat::Event) -> Self {
        ChatBehaviourEvent::Autonat(event)
    }
}

impl From<relay::Event> for ChatBehaviourEvent {
    fn from(event: relay::Event) -> Self {
        ChatBehaviourEvent::RelayServer(event)
    }
}

impl From<relay::client::Event> for ChatBehaviourEvent {
    fn from(event: relay::client::Event) -> Self {
        ChatBehaviourEvent::RelayClient(event)
    }
}

impl From<dcutr::Event> for ChatBehaviourEvent {
    fn from(event: dcutr::Event) -> Self {
        ChatBehaviourEvent::Dcutr(event)
    }
}

impl From<gossipsub::Event> for ChatBehaviourEvent {
    fn from(event: gossipsub::Event) -> Self {
        ChatBehaviourEvent::Gossipsub(event)
    }
}

impl From<kad::Event> for ChatBehaviourEvent {
    fn from(event: kad::Event) -> Self {
        ChatBehaviourEvent::Kademlia(event)
    }
}