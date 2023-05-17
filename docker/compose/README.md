# Ephemera docker image

## Prerequisites

- [Docker](https://docs.docker.com/install/)

## Build

```bash
cd ../../node
docker build . -f ../docker/compose/Dockerfile -t ephemera:latest
```

## Run with docker compose

```bash
docker-compose up
```