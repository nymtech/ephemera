# Ephemera peer discovery example using HTTP API

This example is a HTTP resource where Ephemera PeerDiscovery HTTP implementation is tested. It allows to change Ephemera
group membership
dynamically and to see how it affects the network behavior.

It expects that local Ephemera cluster is running.

## How it works

`HTTP PeerDiscovery` should be configured with short interval so that the cluster can react quickly to the changes.

Full Ephemera cluster is running and is configured to use PeerDiscovery HTTP implementation which connects to this
example.

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
  cargo run -- --help
```

```text
Ephemera http and ws external interfaces example

Usage: ephemera-http-ws-example [OPTIONS] --host <HOST> --http-port <HTTP_PORT> --ws-port <WS_PORT>

Options:
      --host <HOST>
          
      --http-port <HTTP_PORT>
          
      --ws-port <WS_PORT>
          
      --messages-frequency-ms <MESSAGES_FREQUENCY_MS>
          [default: 1000]
      --block-query-frequency-sec <BLOCK_QUERY_FREQUENCY_SEC>
          [default: 10]
  -h, --help
          Print help information
```

```bash
  cargo run -- --nr-of-nodes 3
```

## Stop the cluster

```bash
../../scripts/local-cluster stop
```