# Example how to use the ephemera with specific application functionality 

Signs protocol message payloads.

## Running

Logs go to _logs_ directory.

```bash
./scripts/run-local-p2p.sh cluster -n 22
./scripts/run-local-p2p.sh run -a examples/signatures-app
```

## Stopping

```bash
./scripts/run-local-p2p.sh stop
```

## Websocket 

After starting the cluster, you can connect to the websocket endpoint of any node in the cluster. 

```bash
websocat ws://127.0.0.1:6001
```