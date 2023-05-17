# Ephemera docker image

## Prerequisites

- [Docker](https://docs.docker.com/install/)

## Build

```bash
cd ../node
docker build . -f ../docker/Dockerfile -t ephemera
```

## Run with docker compose

```bash
docker-compose up
```