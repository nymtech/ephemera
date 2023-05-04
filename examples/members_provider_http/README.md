# Ephemera members provider example using HTTP API

This example implements an API which provides Ephemera membership info over http. It allows to change Ephemera group 
membership dynamically and to see how it affects the network behavior.

It expects that local Ephemera cluster is running.

## How it works

Nodes' `members-provider-delay` should be configured with short interval so that the cluster can react quickly to the changes.

Full Ephemera cluster is running and is configured to use HTTP members provider implementation which connects to this
example.

Peers config is read from `~/.ephemera/peers.toml`.

### Scenario 1 - Checking how the cluster behaves when a single node is removed from the cluster

When a node is removed from the cluster:
1)It should stop creating new blocks
2)The rest of the cluster should remove it from the group

When a node is added to the cluster:
1)It should start creating new blocks
2)The rest of the cluster should add it to back to the group

### Scenario 2 - Checking how the cluster behaves when a below threshold number of nodes is removed from the cluster

When enough nodes are removed from the cluster:
1)The whole cluster should stop creating new blocks

When enough nodes are added back to the cluster:
1)The cluster should start creating new blocks again

## How to run

### Create Ephemera cluster (if you don't have one)

```bash
../../scripts/local-cluster init -n 3
```

### Start the cluster

**PS! By default, all nodes produce blocks. To turn it off for a node, edit configuration:**

```text
[block_config]
producer = false
```

From the top-level directory:

```bash
../../scripts/local-cluster run -a ephemera
```

### Run the example
```bash
cargo run -- -h

Ephemera members provider http example

Usage: members_provider_http <--all|--reduced <REDUCED>|--healthy>

Options:
      --all
          
      --reduced <REDUCED>
          
      --healthy
          
  -h, --help
          Print help
```

```bash
  cargo run -- --all
```