# Scripts

## Create new cluster

Creates configuration for new cluster of nodes in ~/.ephemera directory

```bash
/run-local-p2p.sh cluster -n 22
``` 

## Start cluster

Runs _ephemera-signatures-app_ on all nodes in cluster. Started process ids are stored in _.pids_ file.

```bash
run-local-p2p.sh run -a ephemera-signatures-app
```

## Stop cluster

Stops _ephemera-signatures-app_ on all nodes in cluster

```bash
run-local-p2p.sh stop
```