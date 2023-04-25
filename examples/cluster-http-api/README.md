# Ephemera cluster test over HTTP API

## How to run

### Create Ephemera cluster (if you don't have one)

```bash
../../scripts/local-cluster init -n 3
```

### Start the cluster

From the top-level directory:

```bash
../../scripts/local-cluster run -a ephemera
```

### Run the example

```bash
  cargo run --bin cluster-http-api -- --help
```

```text
Ephemera cluster http, meant to test cluster behaviour over http api.

Usage: cluster-http-api [OPTIONS] --nr-of-nodes <NR_OF_NODES>

Options:
      --nr-of-nodes <NR_OF_NODES>
          Ephemera cluster size
      --messages-post-frequency-ms <MESSAGES_POST_FREQUENCY_MS>
          Messages submitting frequency in ms [default: 3000]
  -h, --help
          Print help
```

```bash
  cargo run --bin cluster-http-api -- --nr-of-nodes 3
```

## Stop the cluster

```bash
../../scripts/local-cluster stop
```