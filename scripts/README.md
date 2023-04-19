# Scripts to run local cluster

## Configuration, Output

All output goes to into $HOME/.ephemera directory.

## Ports

Ports are rather arbitrary at the moment. They are incremented by 1 for each node.

See in `scripts/local-cluster`:
```text
WS_PORT=6000
EPHEMERA_PORT=3000
HTTP_API_PORT=7000
```

## Build

Run:
```bash
./local-cluster
```
```text
Please specify a subcommand of:
`init`
`run`
`run_node`
`stop`
`clear`
`delete`
`update_config`
`stop_node`

```

## Create new cluster

Creates configuration for new cluster of nodes in `~/.ephemera` directory.

```bash
./local-cluster init -n 3
```

## Start cluster with Ephemera + Simulated Nym Api

```bash
./local-cluster run -a nym-api
```

## Start cluster with plain Ephemera

```bash
./local-cluster run -a ephemera
```

## Stop cluster

```bash
./local-cluster stop
```


## Clean cluster generated data

```bash
./local-cluster clear
```

## Delete cluster configuration and data

```bash
./local-cluster delete
```

## Update cluster configuration

```bash
./local-cluster update_config -n node1 -k block.producer -v false
```

## Start single node

```bash
./local-cluster run_node -n node1
```

## Stop single node

```bash
./local-cluster stop_node -n node1
```