# Scripts

## Output

All output goes to into $PROJECT_ROOT/cluster directory.

* `cluster/db` - database files
* `cluster/logs` - logs
* `.pids` - process ids

## Create new cluster

Creates configuration for new cluster of nodes in `~/.ephemera` directory

```bash
./run-local-p2p.sh cluster -n 3
``` 

## Start cluster

```bash
./run-local-p2p.sh run
```

## Stop cluster

```bash
./run-local-p2p.sh stop
```

