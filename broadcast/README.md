# Trying out how to implement broadcast and consensus protocols

The goal is to try out something what might be called reliable broadcast and consensus protocols.
No special care is taken yet to make these robust, performant, secure, etc.
The main goal was to get some ideas on the general approach.

## Design overview

The main design goal when implementing a protocol was to keep it encapsulated state machine which is a plain function of its inputs. 
That makes reasoning and testing much easier.

A protocol doesn't know anything about networking, timeouts, etc. All its state transitions are driven by its inputs.

Time concept for a protocol doesn't mean physical time. Point in time is just another event(tick) for it. Tick is ordered relative to
other inputs based on the ordering function(in case of time, natural ordering of the timestamps of the messages)

If it needs to make progress based on time then time is just another input. It's not implemented yet but something like
Clock or Synchronizer can send it special messages(let's call them ticks) to make it progress even if no other messages arrive.

### Synchronizing access by multiple threads

No locks are used even though protocols are accessed by multiple threads(connections) because they process messages in the order they
arrive over the single channel from network.

This also means synchronous processing and extra coping the data when it's send over the channel.

## Networking

`Network` trait is implemented by both basic networking and also for libp2p stack.

[Rust doc](src/network/mod.rs)

### Basic implementation

Networking is very basic `send and forget` system. It opens a new connection for every request.
It also doesn't have a notion of timeouts, acknowledgements and retries. Nor has network level encryption.

 - `NetworkListener`: listens for incoming connection requests and sends protocol messages to network
 - `ConnectionHandler`: listens incoming messages per connection and forwards them to ProtocolHandler
 - `PeerDiscovery`: knows about the peers participating in the protocol. Right now it's implemented as a static list of peers:
   - `StaticPeerDiscovery` - reads the list of peers from configuration file

#### Peers identity

Currently, peers are simply identified by their id in a message. It could be improved with actual PKI and membership.

### Libp2p implementation

libp2p Swarm implements Network trait like the basic implementation.

It uses libp2p's `Gossipsub protocol` for broadcasting and listening gossip messages. For network security it uses libp2p default `Noise protocol`.

Similarly to the basic implementation, it uses `StaticPeerDiscovery` to know about the peers participating in the protocol. 
This should be replaced with an actual discovery protocol(`libp2p's Kademlia DHT`).

#### Peers identity

libp2p `PeerId` is created using peer's public key. 

### Messages encoding

Network messages are encoded using `protobuf`. The protobuf definition are in `proto/broadcast.proto`.

### Protocol

- `ProtocolHandler`: receives messages from the network and passes them to the protocol
- `Protocol`: simple trait that defines the protocol: `request->response model`.

### QuorumConsensusBroadcastProtocol
[Rust doc](src/protocol/implementations/quorum_consensus/protocol)

This a basic implementation of a protocol where participating peers go through three rounds to reach a consensus about if/when deliver a message.

### QuorumConsensusCallBack
[Rust doc](src/protocol/implementations/quorum_consensus/quorum_consensus_callback.rs)

A trait which functions are called as part of QuorumConsensusBroadcastProtocol process and can provide 
custom logic how to process message payload.

### SigningQuorumConsensusCallBack
[Rust doc](src/app/signatures/callback.rs)

Implements a callback for the QuorumConsensusBroadcastProtocol. Signs protocol message payloads. 

It uses `SignaturesBackend` to write signatures from completed rounds to file.

### Crypto for signing

It uses [ed25519-zebra](https://crates.io/crates/ed25519-zebra) crate to sign messages.

### FullGossipProtocol
[Rust doc](src/protocol/implementations/gossip/protocol)

Simple gossip protocol which broadcast a message to all peers first time when it ses it. 

### Configuration

`./configuration` directory contains configuration files for 3 nodes for testing.

## Running a protocol instance

```bash
RUST_LOG="debug" cargo run -- --config-file config1.toml
```

