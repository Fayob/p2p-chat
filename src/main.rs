use anyhow::Ok;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::Mode;
use libp2p::ping::Config;
use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::{identify, kad, mdns, noise, ping, request_response, tcp, yamux, Multiaddr, PeerId, StreamProtocol};
use libp2p::request_response::json;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::{io, select};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageRequest {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageResponse {
    pub ack: bool,
}

#[derive(NetworkBehaviour)]
struct ChatBehaviour {
    ping: ping::Behaviour,
    messaging: json::Behaviour<MessageRequest, MessageResponse>,
    mdns: Toggle<mdns::Behaviour<tokio>>,
    identify: identify::Behaviour,
    kademlia: kad::Behaviour<MemoryStore>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mdns_enabled = env::var("CHAT_MDNS_ENABLED")?.parse::<bool>()?;
    let bootstrap_peers = env::var("CHAT_BOOTSTRAP_PEERS").map(|peers| peers.split(',').collect::<Vec<String>>());
    // let bootstrap_peers = bootstrap_peers.split(",").collect::<Vec<_>>();

    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            // Default::default(),
            tcp::Config::default(), 
            noise::Config::new,
            yamux::Config::default(),
        )?
        .with_behaviour(|key_pair| {
            let mdns = if mdns_enabled {
                Toggle::from(Some(mdns::Behaviour::new(mdns::Config::default(), key_pair.public().to_peer_id())?))
            } else {
                Toggle::from(None)
            };

            let mut kad_config = kad::Config::new(StreamProtocol::new("/p2p-chat/1"));
            kad_config.set_periodic_bootstrap_interval(Some(Duration::from_secs(10)));

            Ok(
                ChatBehaviour {
                    ping: ping::Behaviour::new(Config::new().with_interval(Duration::from_secs(10))),
                    messaging: json::Behaviour::new(
                        [(
                            StreamProtocol::new("/p2p-chat/1"),
                            request_response::ProtocolSupport::Full
                        )],
                        request_response::Config::default(),
                    ),
                    mdns,
                    identify: identify::Behaviour::new(identify::Config::new(
                        "1.0.0".to_string(),
                        key_pair.public(),
                    )),
                    kademlia: kad::Behaviour::with_config(
                        key_pair.public().to_peer_id(), 
                        MemoryStore::new(key_pair.public().to_peer_id()), 
                        kad_config
                    )
                }
            )
        })?
        .with_swarm_config(|config| { config.with_idle_connection_timeout(Duration::from_secs(30))})
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    swarm.behaviour_mut().kademlia.set_mode(Some(Mode::Server));

    println!("Peer ID: {:?}", swarm.local_peer_id());

    if let Ok(bootstrap_peers) = bootstrap_peers {
        for bootstrap_peer in bootstrap_peers {
            let addr: Multiaddr = bootstrap_peer.parse()?;
            let peer_id = addr.iter().map(|addr_str| {
                if let Some(Protocol::P2p(peer_id)) = addr.iter().last() {
                    return Some(peer_id);
                }
                None
            })
                .filter(Option::is_some)
                .last()
                .ok_or(anyhow("No peer ID found in address!"))?
                .ok_or(anyhow("Bootstrap peer address {bootstrap_peer} is wrong"))?;
            swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
        }
    }

    let mut stdin = BufReader::new(io::stdin()).lines();

    loop {
        select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::NewListenAddr {address, ..} => {
                    println!("Listening on {:?}", address);
                }
                SwarmEvent::ConnectionEstablished {peer_id, ..} => {
                    println!("Connection established with peer {:?}", peer_id);
                }
                SwarmEvent::Behaviour(event) => match event {
                    ChatBehaviourEvent::Ping(event) => {
                        println!("Ping event: {:?}", event);
                    },
                    ChatBehaviourEvent::Messaging(event) => match event {
                        request_response::Event::Message{peer, message} => match message {
                            request_response::Message::Request{request_id, request, channel } => {
                                println!("{peer} {:?}", request.message);
                                if let Err(error) = swarm.behaviour_mut().messaging.send_response(channel, MessageResponse { ack: true}) {
                                    println!("Error sending response: {:?}", error);
                                }
                            }
                            request_response::Message::Response{ request_id, response } => {
                                println!("{peer} Response ACK: {:?}", response.ack);
                            },
                        },
                        request_response::Event::OutboundFailure{peer, request_id, error} => {
                            println!("OutboundFailure from {:?} to {:?}: {:?}", peer, request_id, error);
                        },
                        request_response::Event::InboundFailure{peer, request_id, error} => {
                            println!("InboundFailure from {:?} to {:?}: {:?}", peer, request_id, error);
                        },
                        request_response::Event::ResponseSent{ .. } => {},
                    }
                    ChatBehaviourEvent::Mdns(event) => match event {
                        mdns::Event::Discovered(new_peers) => {
                            for (peer_id, addr) in new_peers {
                                println!("Discovered {peer_id} at {addr}!");
                                swarm.dial(addr.clone())?;
                                swarm.add_peer_address(peer_id, addr);
                            }
                        }
                        mdns::Event::Expired(_) => {}
                    }
                    ChatBehaviourEvent::Identify(event) => match event {
                        identify::Event::Received { connection_id, peer_id, info } => {
                            println!("Received identify info from {peer_id}: {:?}", info);
                            for addr in info.listen_addrs {
                                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);

                            }
                        }
                        identify::Event::Sent { connection_id, peer_id } => {
                            println!("Sent identify info to {peer_id}");
                        }
                        identify::Event::Pushed { connection_id, peer_id, info } => {
                            println!("Sent identify info to {peer_id}");
                        }
                        identify::Event::Error { connection_id, peer_id, error } => {
                            println!("Sent identify info to {peer_id}");
                        }
                    }
                    ChatBehaviourEvent::Kademlia(event) => match event {
                        kad::Event::InboundRequest { .. } => {}
                        kad::Event::OutboundQueryProgressed {..} => {}
                        kad::Event::RoutingUpdated { peer, is_new_peer, addresses, bucket_range, old_peer} => {
                            println!("New routing uodate! Peer {peer} - {addresses:?}");
                            addresses.iter().for_each(|addr| {
                                if let Err(error) = swarm.dial(addr.clone()) {
                                    println!("Error dialing address {:?}: {:?}", addr, error);
                                }
                            })
                        }
                        kad::Event::UnroutablePeer { .. } => {}
                        kad::Event::RoutablePeer { .. } => {}
                        kad::Event::PendingRoutablePeer { .. } => {}
                        kad::Event::ModeChanged { .. } => {}
                    }
                }
                _ => {}
            },
            Ok(Some(line)) = stdin.next_line() => {
                let peers = swarm.connected_peers().copied().collect::<Vec<PeerId>>();
                for peer_id in peers {
                    swarm.behaviour_mut().messaging.send_request(&peer_id, MessageRequest { message: line.clone()});
                    println!("{} {line:?}". swarm.local_peer_id();)
                }
            }
        }
    }
}
