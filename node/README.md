# Ephemera - reliable broadcast protocol

Ephemera does reliable broadcast for blocks.

## Short Overview

All Ephemera nodes accept messages submitted by clients. Node then gossips these to other nodes in the cluster. After certain interval,
a node collects messages and produces a block. Then it does reliable broadcast for the block with other nodes in the cluster.

Ephemera doesn't have the concept of leader(at the moment).  It's up to an 'Application' to decide which block to use. 
For example in case of Nym-Api, it is the first block submitted to a "Smart Contract".

At the same time, the purpose of blocks is to reach to consensus about which messages are included. It's just that Ephemera doesn't make the final decision,
instead it leaves that to an application.

## Main concepts

- **Node** - a single instance of Ephemera.
- **Cluster** - a set of nodes participating in reliable broadcast.
- **EphemeraMessage** - a message submitted by a client.
- **Block** - a set of messages collected by a node.
- **Application(ABCI)** - a trait which Ephemera users implement to accept messages and blocks.
  - check_tx
  - check_block
  - accept_block

## How to run

[README](../scripts/README.md)

## HTTP API

See [Rust](src/api/http/mod.rs)

- `/ephemera/node/health`
- `/ephemera/broadcast/block/{hash}`
- `/ephemera/broadcast/block/certificates/{hash}`
- `/ephemera/broadcast/block/height/{height}`
- `/ephemera/broadcast/blocks/last`
- `/ephemera/node/config`
- `/ephemera/dht/query/{key}`

- `/ephemera/broadcast/submit_message`
- `/ephemera/dht/store`

## Rust API

See [Rust](src/api/mod.rs)

## Application(Ephemera ABCI)

See [Rust](src/api/application.rs)

## Examples

### Ephemera HTTP and WS external interfaces example

See [README.md](../examples/http-ws-sync/README.md)

### Nym Api simulation

See [README.md](../examples/nym-api/README.md)

### http API example

See [README.md](../examples/cluster-http-api/README.md)

### Membership over HTTP API

See [README.md](../examples/members_provider_http/README.md)