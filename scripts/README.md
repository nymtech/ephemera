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
./scripts/local-cluster init -n 3
``` 

## Start cluster

```bash
./scripts/local-cluster run
tail -f cluster/logs/ephemera1.log
```

## Stop cluster

```bash
./scripts/local-cluster stop
```

