# Ephemera embedded API demonstration

This example connects to an Ephemera node.

1) Submits signed messages to the node
2) Listens WS for new blocks
3) Queries the node for received blocks over http API and compares them with the ones received via WS
4) Verifies the signatures of the blocks
5) Verifies the signatures of the messages in the blocks

## How to run

### Create Ephemera cluster(if you don't have one)

```bash
 ../../scripts/run-local-p2p.sh cluster -n 3
```

### Start the cluster 

**PS! By default, all nodes produce blocks. To turn it off for a node, edit configuration:**
```text
[block_config]
leader = false
```

```bash
 ../../scripts/run-local-p2p.sh run
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
  cargo run -- --host 127.0.0.1 --http-port 7001 --ws-port 6001
```

### Monitor server logs

```bash
 tail -f ../../cluster/logs/ephemera1.log
```

```bash
 tail -f ../../cluster/logs/ephemera2.log
```

```bash
 tail -f ../../cluster/logs/ephemera3.log
```

## Stop the cluster

```bash
 ../../scripts/run-local-p2p.sh stop
```