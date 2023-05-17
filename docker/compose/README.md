# Ephemera docker image

## Prerequisites

In `~/.ephemera/peers.toml` peer address format should be `/dns4/node1/tcp/3000`, `/dns4/node2/tcp/3001` 
instead of `/ip4/127.0.0.1/tcp/3000`, `/ip4/127.0.0.1/tcp/3001` and so on.

In each node config file `~/.ephemera.toml` node ip should be `0.0.0.0`.
In each node config file `~/.ephemera.toml` sqlite db file should be `/ephemera.sqlite`

## Build

```bash
cd ../node
docker build . -f ../docker/Dockerfile -t ephemera
```

## Run with docker compose

```bash
docker-compose up
```