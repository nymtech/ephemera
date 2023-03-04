# Scripts

## Configuration, Output

All output goes to into $HOME/.ephemera directory.

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
./scripts/local-cluster stop
```

