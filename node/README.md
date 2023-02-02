## Running a node instance

```bash
RUST_LOG="debug" cargo run -- --config-file config1.toml
```

See also [scripts/README.md](../scripts/README.md).

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


## Networking

### Libp2p implementation

It uses libp2p's `Gossipsub protocol` for broadcasting and listening gossip messages. For network security it uses libp2p default `Noise protocol`.


### Peers

It uses `StaticPeerDiscovery` to know about the peers participating in the protocol. 
This should be replaced with an actual discovery protocol(`libp2p's Kademlia DHT`).

#### Peers identity

libp2p `PeerId` is created using peer's public key. 

### Messages encoding

Network messages are encoded using `protobuf`. The protobuf definition are in `proto/broadcast.proto`.

### BroadcastProtocol
[Rust doc](src/broadcast_protocol/broadcast.rs)

This a basic implementation of a protocol where participating peers go through three rounds to reach a consensus about if/when deliver a message.

### BroadcastCallBack
[Rust doc](src/broadcast_protocol/mod.rs)

A trait which functions are called as part of QuorumConsensusBroadcastProtocol process and can provide 
custom logic how to process message payload.

### Crypto for signing

It uses [ed25519-zebra](https://crates.io/crates/ed25519-zebra) crate to sign messages.