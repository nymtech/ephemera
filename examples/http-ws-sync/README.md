# Ephemera embedded API demonstration / test to verify blocks 

This example connects to an Ephemera node.

## How it works

1) Submits signed messages to the node
2) Listens WS for new blocks
3) Queries the node for received blocks over http API and compares them with the ones received via WS
4) Verifies the signatures of the blocks
5) Verifies the signatures of the messages in the blocks

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
cargo run -- --host 127.0.0.1 --http-port 7001 --ws-port 6001 --messages-frequency-ms 10000
```

## Stop the cluster

```bash
../../scripts/local-cluster stop
```

## Example output

```text
Connecting to ephemera node on 127.0.0.1

Querying new blocks 10 ms

Listening to ws blocks on ws://127.0.0.1:6001

Sending messages to http://127.0.0.1:7001

Sending signed messages every 10000 ms

Received new block
Received nr of blocks: 1

Block found by hash: "aFEUaCYfP8iBNwQy3EVjgfE1LmojibGr56Q9vx39cuu"

Comparing block aFEUaCYfP8iBNwQy3EVjgfE1LmojibGr56Q9vx39cuu

Block header match
Comparing block messages, count: 103
Block messages match
Verifying messages signatures: 103

All messages signatures are valid
Block certificates found by hash: "aFEUaCYfP8iBNwQy3EVjgfE1LmojibGr56Q9vx39cuu", len 6

Verifying block certificates: 6

Certificate from peer 12D3KooWQrvPUr9kFM8KkVoWwgUT13NXaFnryVa6RzDJAxYJMnV9 is valid
Certificate from peer 12D3KooWBGdcpj361Zpez5Psjdz8U11D69wvTThphqXCLQZHxqWT is valid
Certificate from peer 12D3KooW9wcn2XUQxfeL5MRnMMpe7ickcjBz1B2aSAfbHPwjHziy is valid
Certificate from peer 12D3KooWNiJyRXhBmKmTWsvZXamedDWBS3rG3ojKKE7tcEGsttgY is valid
Certificate from peer 12D3KooWJSZzExhcoP1F2k1yBbEkkpSyMHMFFkq6KSnESU8WC334 is valid
Certificate from peer 12D3KooWRfPuqp65cKBT8d5k3ECbpVZMrbrK4GhpW2QfNxyPBuRv is valid
```