use uuid::Uuid;
use std::collections::HashSet;
use anyhow::anyhow;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::Mode;
use libp2p::ping::Config;
use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent}; // Removed `self`
use libp2p::multiaddr::Protocol;
use libp2p::futures::StreamExt;
use libp2p::{
    identify,
    kad,
    noise,
    ping,
    request_response,
    tcp,
    yamux,
    Multiaddr,
    PeerId,
    StreamProtocol,
};
use libp2p::mdns;
use libp2p::request_response::json;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::{io, select};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageRequest {
    pub id: Uuid,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageResponse {
    pub ack: bool,
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ChatBehaviourEvent")]
struct ChatBehaviour {
    ping: ping::Behaviour,
    messaging: json::Behaviour<MessageRequest, MessageResponse>,
    mdns: Toggle<mdns::tokio::Behaviour>,
    identify: identify::Behaviour,
    kademlia: kad::Behaviour<MemoryStore>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
enum ChatBehaviourEvent {
    Ping(ping::Event),
    Messaging(request_response::Event<MessageRequest, MessageResponse>),
    Mdns(mdns::Event),
    Identify(identify::Event),
    Kademlia(kad::Event),
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

impl From<kad::Event> for ChatBehaviourEvent {
    fn from(event: kad::Event) -> Self {
        ChatBehaviourEvent::Kademlia(event)
    }
}


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mdns_enabled = env::var("CHAT_MDNS_ENABLED")
        .map(|s| s.parse::<bool>().unwrap_or(false))
        .unwrap_or(false);

    let bootstrap_peers_str = env::var("CHAT_BOOTSTRAP_PEERS").ok();

    let local_key = libp2p::identity::Keypair::generate_ed25519();
    let local_peer_id = local_key.public().to_peer_id();

    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(move |key| {
            let peer_id = key.public().to_peer_id();

            let mdns = if mdns_enabled {
                Toggle::from(Some(mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?))
            } else {
                Toggle::from(None)
            };

            let mut kad_config = kad::Config::new(StreamProtocol::new("/p2p-chat/1"));
            kad_config.set_periodic_bootstrap_interval(Some(Duration::from_secs(10)));
            kad_config.set_query_timeout(Duration::from_secs(60));

            Ok(ChatBehaviour {
                ping: ping::Behaviour::new(Config::new().with_interval(Duration::from_secs(10))),
                messaging: json::Behaviour::new(
                    [(
                        StreamProtocol::new("/p2p-chat/1"),
                        request_response::ProtocolSupport::Full,
                    )],
                    request_response::Config::default(),
                ),
                mdns,
                identify: identify::Behaviour::new(identify::Config::new(
                    "1.0.0".to_string(),
                    key.public(),
                )),
                kademlia: kad::Behaviour::with_config(
                    peer_id,
                    MemoryStore::new(peer_id),
                    kad_config,
                ),
            })
        })?
        .with_swarm_config(|c| {
            c.with_idle_connection_timeout(Duration::from_secs(30))
        })
        .build();

    swarm.behaviour_mut().kademlia.set_mode(Some(Mode::Server));

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    println!("Local Peer ID: {}", swarm.local_peer_id());

    if let Some(peers_str) = bootstrap_peers_str {
        for bootstrap_peer_multiaddr_str in peers_str.split(',') {
            let addr: Multiaddr = bootstrap_peer_multiaddr_str.parse()
                .map_err(|e| anyhow!("Failed to parse bootstrap multiaddr '{}': {}", bootstrap_peer_multiaddr_str, e))?;

            let peer_id = if let Some(Protocol::P2p(peer_id_bytes)) = addr.iter().last() {
                PeerId::try_from(peer_id_bytes)
                    .map_err(|e| anyhow!("Invalid PeerId in bootstrap address '{}': {}", bootstrap_peer_multiaddr_str, e))?
            } else {
                return Err(anyhow!("Bootstrap peer address '{}' does not end with a /p2p/ PeerId component.", bootstrap_peer_multiaddr_str));
            };
            
            println!("Adding bootstrap peer: {} at {}", peer_id, addr);
            swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
        }
    } else {
        println!("No CHAT_BOOTSTRAP_PEERS environment variable found. Relying on mDNS or manual connections.");
    }


    let mut stdin = BufReader::new(io::stdin()).lines();
    let mut seen_messages = HashSet::new();

    loop {
        select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on {:?}", address);
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    println!("Connection established with peer {:?}", peer_id);
                }
                SwarmEvent::Behaviour(event) => match event {
                    ChatBehaviourEvent::Ping(event) => {
                        // println!("Ping event: {:?}", event);
                    },
                    ChatBehaviourEvent::Messaging(event) => match event {
                        request_response::Event::Message { peer, message, .. } => match message {
                            request_response::Message::Request { request_id, request, channel } => {
                                if seen_messages.insert(request.id) {
                                    println!("Received from {}: {}", peer, request.message);
                                    // Gossip to all peers except the sender
                                    let peers: Vec<_> = swarm.connected_peers().copied().filter(|p| *p != peer).collect();
                                    for other_peer in peers {
                                        swarm.behaviour_mut().messaging.send_request(&other_peer, request.clone());
                                    }
                                }
                                if let Err(error) = swarm.behaviour_mut().messaging.send_response(channel, MessageResponse { ack: true }) {
                                    println!("Error sending response: {:?}", error);
                                }
                            }
                            request_response::Message::Response { request_id, response } => {
                                println!("Response from {}: ACK {:?}", peer, response.ack);
                            },
                        },
                        request_response::Event::OutboundFailure { peer, request_id, error } => {
                            eprintln!("OutboundFailure to {:?} (req {}): {:?}", peer, request_id, error);
                        },
                        request_response::Event::InboundFailure { peer, request_id, error } => {
                            eprintln!("InboundFailure from {:?} (req {}): {:?}", peer, request_id, error);
                        },
                        request_response::Event::ResponseSent { .. } => {},
                    }
                    ChatBehaviourEvent::Mdns(event) => match event {
                        mdns::Event::Discovered(new_peers) => {
                            for (peer_id, addr) in new_peers {
                                println!("mDNS Discovered {peer_id} at {addr}!");
                                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                                if let Err(e) = swarm.dial(addr.clone()) {
                                    eprintln!("Error dialing mDNS discovered peer {}: {:?}", peer_id, e);
                                }
                            }
                        }
                        mdns::Event::Expired(expired_peers) => {
                             // println!("mDNS Expired peers: {:?}", expired_peers);
                        }
                    }
                    ChatBehaviourEvent::Identify(event) => match event {
                        identify::Event::Received { connection_id, peer_id, info } => {
                            println!("Identify: Received info from {}: {:?}", peer_id, info.agent_version);
                            for addr in info.listen_addrs {
                                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                            }
                        }
                        identify::Event::Sent { connection_id, peer_id } => {
                            // println!("Identify: Sent info to {}", peer_id);
                        }
                        identify::Event::Pushed { .. } => {}
                        identify::Event::Error { .. } => {}
                    }
                    ChatBehaviourEvent::Kademlia(event) => match event {
                        kad::Event::InboundRequest { .. } => {}
                        kad::Event::OutboundQueryProgressed {..} => {}
                        kad::Event::RoutingUpdated { peer, is_new_peer, addresses, ..} => {
                            if is_new_peer {
                                println!("Kademlia: New routing update! Discovered peer {}: {:?}", peer, addresses);
                                for addr in addresses.iter() {
                                    swarm.behaviour_mut().kademlia.add_address(&peer, addr.clone());
                                }
                            }
                        }
                        kad::Event::UnroutablePeer { .. } => {}
                        kad::Event::RoutablePeer { .. } => {}
                        kad::Event::PendingRoutablePeer { .. } => {}
                        kad::Event::ModeChanged { .. } => {}
                    }
                }
                _ => {}
            },
            result = stdin.next_line() => {
                let line = match result {
                    Ok(Some(line)) => line,
                    Ok(None) => {
                        println!("Stdin closed, exiting.");
                        break;
                    },
                    Err(e) => {
                        eprintln!("Error reading from stdin: {}", e);
                        continue;
                    }
                };

                if line.trim().is_empty() {
                    continue;
                }

                let connected_peers = swarm.connected_peers().copied().collect::<Vec<PeerId>>();
                if connected_peers.is_empty() {
                    println!("No connected peers to send message to.");
                } else {
                    println!("Sending '{}' to {} connected peers.", line, connected_peers.len());
                    for peer_id in connected_peers {
                        swarm.behaviour_mut().messaging.send_request(&peer_id, MessageRequest { id: Uuid::new_v4(), message: line.clone() });
                    }
                }
            }
        }
    }

    Ok(())
}