# Peer-To-Peer (P2p) Chat

A simple peer-to-peer (P2P) chat application built in Rust using [libp2p](https://libp2p.io/).  
This project demonstrates decentralized messaging, peer discovery, and message gossiping over a local network or the internet.

---

## Features

- **Decentralized:** No central server; all nodes are equal.
- **Peer Discovery:** Uses [Kademlia DHT](https://docs.libp2p.io/concepts/dht/) and [mDNS](https://en.wikipedia.org/wiki/Multicast_DNS) for finding peers.
- **Secure:** Uses [Noise](https://noiseprotocol.org/) for encrypted connections.
- **Multiplexed:** Uses [Yamux](https://github.com/hashicorp/yamux) for stream multiplexing.
- **Gossip Protocol:** Messages are broadcast to all peers using a simple gossip mechanism.
- **Request/Response Messaging:** Uses libp2p's request-response protocol with JSON serialization.
- **Ping & Identify:** Supports pinging peers and exchanging identity information.

---

## How It Works

- Each peer generates a unique identity on startup.
- Peers discover each other via mDNS (on LAN) or via bootstrap addresses (over the internet).
- When you type a message, it is sent to all currently connected peers.
- When a peer receives a new message (determined by a unique message ID), it gossips the message to all its connected peers except the sender.
- Each peer keeps track of seen messages to avoid rebroadcasting the same message.

---

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (edition 2021 or later)
- [Cargo](https://doc.rust-lang.org/cargo/)

### Installation

Clone the repository:

```sh
git clone https://github.com/Fayob/p2p-chat.git
cd p2p-chat
```

Install dependencies and build:

```sh
cargo build --release
```

---

## Usage

### Start the First Peer

```sh
cargo run
```

- The peer will print its listening address and Peer ID.
- Example output:
  ```
  Local Peer ID: 12D3KooW...
  Listening on "/ip4/127.0.0.1/tcp/12345/p2p/12D3KooW..."
  ```

### Start Additional Peers and Connect

Open a new terminal for each additional peer and run:

```sh
CHAT_BOOTSTRAP_PEERS="/ip4/127.0.0.1/tcp/12345/p2p/12D3KooW..." cargo run
```

- Replace the address with the one printed by the first peer.
- You can provide multiple addresses separated by commas.

### LAN Discovery

If you are on the same local network, peers will discover each other automatically via mDNS (unless you disable it).

---

## Environment Variables

- `CHAT_MDNS_ENABLED`  
  - Set to `true` (default) to enable mDNS peer discovery.
  - Set to `false` to disable mDNS.
- `CHAT_BOOTSTRAP_PEERS`  
  - Comma-separated list of multiaddresses to connect to at startup.

Example:

```sh
CHAT_MDNS_ENABLED=false CHAT_BOOTSTRAP_PEERS="/ip4/192.168.1.10/tcp/4001/p2p/12D3KooW..." cargo run
```

---

## How to Use

- Type a message and press Enter to send it.
- All connected peers will receive and display the message.
- Messages are gossiped through the network, so even indirectly connected peers will eventually receive them.

---

## Code Structure

- **main.rs**  
  - Sets up the libp2p Swarm with Kademlia, mDNS, Ping, Identify, and Request/Response protocols.
  - Handles peer discovery, connection management, and message gossiping.
  - Reads user input and sends messages to peers.
  - Deduplicates messages using a UUID and a `HashSet`.

---

## Example Output

```
Local Peer ID: 12D3KooW...
Listening on "/ip4/127.0.0.1/tcp/12345/p2p/12D3KooW..."
Connection established with peer 12D3KooX...
Received from 12D3KooX...: Hello, world!
Sending 'Hi there!' to 1 connected peers.
Response from 12D3KooX...: ACK true
```

---

## Troubleshooting

- **No connected peers:**  
  - Make sure at least one peer is running and reachable.
  - Check your firewall settings.
  - Use the correct bootstrap address.
- **Messages not received:**  
  - Ensure all peers are connected (check logs for connection events).
  - If using mDNS, ensure all peers are on the same LAN/subnet.

---

## License

MIT

---

## Acknowledgments

- [libp2p](https://libp2p.io/)
- [Rust](https://www.rust-lang.org/)