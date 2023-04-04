# Ephemera cluster test over HTTP API

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
Ephemera cluster http, meant to test cluster behaviour over http api

Usage: cluster-http-api [OPTIONS] --nr-of-nodes <NR_OF_NODES>

Options:
      --nr-of-nodes <NR_OF_NODES>
          
      --messages-post-frequency-ms <MESSAGES_POST_FREQUENCY_MS>
          [default: 1000]
      --block-query-frequency-sec <BLOCK_QUERY_FREQUENCY_SEC>
          [default: 10]
  -h, --help
          Print help

```

```bash
  cargo run -- --host 127.0.0.1 --http-port 7001 --ws-port 6001
```

## Stop the cluster

```bash
../../scripts/local-cluster stop
```