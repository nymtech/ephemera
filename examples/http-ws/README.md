# Ephemera embedded API demonstration

This example connects to a Ephemera node.

1) Submits signed messages to the node
2) Listens WS for new blocks
3) Queries the node for received blocks oer http API and compares them with the ones received via WS
4) Verifies the signatures of the blocks
5) Verifies the signatures of the messages in the blocks

## How to run

### Create Ephemera cluster(if you don't have one)

```bash
 ../../scripts/run-local-p2p.sh cluster -n 3
```

## Start the cluster 

```bash
 ../../scripts/run-local-p2p.sh run
```

## Run the example

```bash
  cargo run -- --host 127.0.0.1 --http-port 7001 --ws-port 6001
```

## Stop the cluster

```bash
 ../../scripts/run-local-p2p.sh stop
```