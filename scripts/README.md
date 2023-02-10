# Scripts

## Output

All output goes to into $PROJECT_ROOT/cluster directory.

* `cluster/db` - database files
* `cluster/logs` - logs
* `.pids` - process ids

## Create new cluster

Creates configuration for new cluster of nodes in `~/.ephemera` directory.

From the top-level directory: 

```bash
./scripts/run-local-p2p.sh cluster -n 3
``` 

## Start cluster

```bash
./scripts/run-local-p2p.sh run
tail -f cluster/logs/ephemera1.log
```

## Stop cluster

```bash
./scripts/run-local-p2p.sh stop
```

