#!/bin/bash

set -e

CLUSTER_HELP=$(
  cat <<-EOH
Initiates new cluster of ephemera nodes.

  usage:      $0 cluster [-n] <number of instances>

  options:
    -n        Number of instances to create
EOH
)

STOP_HELP=$(
  cat <<-EOH
Stops currently running cluster of ephemera nodes.

  usage:      $0 stop

EOH
)

SUBCMD=$1

[[ $SUBCMD == "cluster" || $SUBCMD == "run" || $SUBCMD == "stop" ]] ||
  { echo "Please specify a subcommand of \`cluster\` or \`run\` or \`stop\`" && exit 1; }

shift

[[ $SUBCMD == "cluster" ]] && [[ $# -lt 1 || $# -gt 2 ]] && echo "$CLUSTER_HELP" && exit 1

[[ $SUBCMD == "cluster" ]] && {
  while getopts :n opt; do
    case $opt in
    n)
      NR_OF_NODES="${2}"
      shift
      ;;
    h)
      echo "$CLUSTER_HELP"
      exit 0
      ;;
    \?)
      echo "Invalid option: $OPTARG" >&2
      exit 1
      ;;
    esac
  done
}


PROJECT_ROOT=$(git rev-parse --show-toplevel)
CLUSTER_DIR="${PROJECT_ROOT}/cluster"
PIDS_FILE=$CLUSTER_DIR/.pids
EPHEMERA="$PROJECT_ROOT"/target/release/ephemera
HOSTNAME=$(hostname)
DB_PATH="$CLUSTER_DIR"/db

export RUST_LOG="debug"

build() {
  echo "Building ephemera..."
  cargo build --release
}

create_cluster() {
  build

  echo "Creating configuration for ${NR_OF_NODES} nodes..."

  COUNTER=1
  for ((c = 1; c <= NR_OF_NODES; c++)); do
    NETWORK_CLIENT_LISTENER_ADDRESS="$HOSTNAME":400"$COUNTER"
    WS_ADDRESS="$HOSTNAME":600"$COUNTER"
    HTTP_SERVER_ADDRESS="$HOSTNAME":700"$COUNTER"
    $EPHEMERA init \
            --node node"$c" \
            --port 300"$c" \
            --db-file "$DB_PATH"/ephemera"$COUNTER".sqlite \
            --ws-address "$WS_ADDRESS" \
            --network-client-listener-address "$NETWORK_CLIENT_LISTENER_ADDRESS" \
            --http-server-address "$HTTP_SERVER_ADDRESS"

    COUNTER=$((COUNTER + 1))
  done
  $EPHEMERA add-local-peers --ephemera-root-dir ~/.ephemera

  status=$?
  [ $status -eq 0 ] && echo "Successfully created cluster" || echo "Creating cluster failed"

}

start_cluster() {
  if test -f "$PIDS_FILE"; then
    echo "Cluster is already running, try stopping it first by executing /run-local-p2p.sh stop."
    exit 1
  fi

  COUNTER=0
  for dir in ~/.ephemera/*/ephemera.toml; do
      COUNTER=$((COUNTER + 1))
  done
  echo $COUNTER

  [[ $COUNTER -lt 1 ]] && echo "No ephemera nodes found, try creating a cluster first." && exit 1

  build

  echo "Starting $COUNTER nodes"

  mkdir -p "$CLUSTER_DIR"
  mkdir -p "$CLUSTER_DIR"/logs
  mkdir -p "$CLUSTER_DIR"/db
  touch "$PIDS_FILE"

  COUNTER=1
  for d in ~/.ephemera/*/ephemera.toml; do

    touch "$DB_PATH"/ephemera"$COUNTER".sqlite

    LOGS_FILE=$CLUSTER_DIR/logs/ephemera$COUNTER.log

    echo "Starting $d"
    $EPHEMERA run-node --config-file ~/.ephemera/node"${COUNTER}"/ephemera.toml > "$LOGS_FILE" 2>&1 &

    echo "$!" >> "$PIDS_FILE"
    COUNTER=$((COUNTER + 1))
  done

  echo "Started $((COUNTER - 1)) ephemera signatures application instances."
  echo "Log files are in $CLUSTER_DIR/logs directory."
}

stop_cluster() {
  if test -f "$PIDS_FILE";
  then
     while read p; do
       kill -9 "$p" || true
     done < "$PIDS_FILE"
     rm "$PIDS_FILE"
     echo "Stopped ephemera cluster"
  else
     echo "No ephemera cluster running"
  fi
}

[[ $SUBCMD == "cluster" ]] && create_cluster
[[ $SUBCMD == "run" ]] && start_cluster
[[ $SUBCMD == "stop" ]] && stop_cluster
