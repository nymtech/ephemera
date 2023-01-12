# Scripts

Run from project root directory.

## Create new cluster

Creates configuration for new cluster of nodes in ~/.ephemera directory

```bash
./scripts/run-local-p2p.sh cluster -n 3
``` 

## Start cluster

Runs _ephemera-signatures-app_ on all nodes in cluster.


```bash
./scripts/run-local-p2p.sh run -a ephemera-signatures-app
```

## Stop cluster

Stops _ephemera-signatures-app_ on all nodes in cluster

```bash
./scripts/run-local-p2p.sh stop
```

## Example websocket output

```json
andrus@Andruss ~ % websocat ws://127.0.0.1:6001
{"request_id":"c8020847-8c80-424e-af7e-22138a3e1780","payload":[80,97,121,108,111,97,100],"signatures":[{"id":"/ip4/127.0.0.1/tcp/3001","signature":"2821392265114658e05367e54b7d6851ce3015fdf08bd8f17f8ee46bfa712a86f01146331fa7bb32ea256441e46115087dbedfbb8ae7154f1bd16f4225c6bd0f"},{"id":"/ip4/127.0.0.1/tcp/3002","signature":"639aa89334c842c462822355605f7db63b42db6ac3c9e6597da62be71d0b81874501b1415a53874e83f3af5393f777ea8be4ea55194d57f399f67f20eabab700"}]}
{"request_id":"c3349e5b-69ce-4bb3-8257-5ad2747f1070","payload":[80,97,121,108,111,97,100],"signatures":[{"id":"/ip4/127.0.0.1/tcp/3002","signature":"639aa89334c842c462822355605f7db63b42db6ac3c9e6597da62be71d0b81874501b1415a53874e83f3af5393f777ea8be4ea55194d57f399f67f20eabab700"},{"id":"/ip4/127.0.0.1/tcp/3001","signature":"2821392265114658e05367e54b7d6851ce3015fdf08bd8f17f8ee46bfa712a86f01146331fa7bb32ea256441e46115087dbedfbb8ae7154f1bd16f4225c6bd0f"}]}
```