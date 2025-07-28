use anyhow::Ok;
use libp2p::ping::Config;
use libp2p::{mdns, noise, ping, request_response, tcp, yamux, Multiaddr, PeerId, StreamProtocol};
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
    mdns: mdns::Behaviour<tokio>
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            // Default::default(),
            tcp::Config::default(), 
            noise::Config::new,
            yamux::Config::default(),
        )?
        .with_behaviour(|key_pair| {
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
                    mdns: mdns::Behaviour::new(mdns::Config::default(), key_pair.public().to_peer_id())?,
                }
            )
        })?
        .with_swarm_config(|config| { config.with_idle_connection_timeout(Duration::from_secs(30))})
        .build();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    println!("Peer ID: {:?}", swarm.local_peer_id());

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
                    request_response::Event::Message{peer, message} => match message {
                        request_response::Message::Request{request_id, request, channel } => {
                            println!("{peer} {:?}", request.message);
                            if let Err(error) = swarm.behaviour_mut().messaging.send_response(channel, MessageResponse { ack: true}) {
                                println!("Error sending response: {:?}", error);
                            }
                        }
                        request_response::Message::Response{ .. } => {},
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
