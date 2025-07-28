use anyhow::Ok;
use libp2p::ping::Config;
use libp2p::{noise, ping, request_response, tcp, yamux, Multiaddr, StreamProtocol};
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let port = std::env::var("CHAT_APP_PORT").unwrap_or(String::from("9999")).parse::<u16>()?;
    let peer: Multiaddr = std::env::var("CHAT_PEER")?.parse()?;

    let mut swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            // Default::default(),
            tcp::Config::default(), 
            noise::Config::new,
            yamux::Config::default(),
        )?
        .with_behaviour(|_key_pair| {
            Ok(
                ChatBehaviour {
                    ping: ping::Behaviour::new(Config::new().with_interval(Duration::from_secs(10))),
                    messaging: json::Behaviour::new(
                        [(
                            StreamProtocol::new("/p2p-chat/1"),
                            request_response::ProtocolSupport::Full
                        )],
                        request_response::Config::default(),
                    )
                }
            )
        })?
        .with_swarm_config(|config| { config.with_idle_connection_timeout(Duration::from_secs(30))})
        .build();

    swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{}", port).parse()?)?;
    swarm.dial(peer.clone())?;

    println!("Peer ID: {:?}", swarm.local_peer_id());

    let mut stdin = BufReader::new(io::stdin()).lines();

    let mut target_peer_id = None;

    loop {
        select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::NewListenAddr {address, ..} => {
                    println!("Listening on {:?}", address);
                }
                SwarmEvent::ConnectionClosed {..} => {
                    target_peer_id = None;
                }
                SwarmEvent::ConnectionEstablished {peer_id, ..} => {
                    println!("Connection established with peer {:?}", peer_id);
                    if target_peer_id.is_none() {
                        target_peer_id = Some(peer_id);
                    }
                    swarm.add_peer_address(peer_id, peer.clone());
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
                _ => {}
            },
            Ok(Some(line)) = stdin.next_line() => {
                if let Some(peer_id) = target_peer_id {
                    swarm.behaviour_mut().messaging.send_request(&peer_id, MessageRequest { message: line.clone()});
                    println!("{} {line:?}". swarm.local_peer_id();)
                }
            }
        }
    }
}
