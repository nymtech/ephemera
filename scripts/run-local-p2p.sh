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

RUN_HELP=$(
  cat <<-EOH
Starts cluster of ephemera nodes.

  usage:      $0 run [-a] <configuration dir>

  options:
    -a        Ephemera application directory
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
[[ $SUBCMD == "run" ]] && [[ $# -lt 1 || $# -gt 2 ]] && echo "$RUN_HELP" && exit 1

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

[[ $SUBCMD == "run" ]] && {
  while getopts :a opt; do
    case $opt in
    a)
      APP_DIR="${2}"
      shift
      ;;
    h)
      echo "${HELP_TXT[$SUBCMD]}"
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
EPHEMERA="$PROJECT_ROOT"/target/release/ephemera
SIGNATURES_APP="$PROJECT_ROOT"/target/release/ephemera-signatures-app
PIDS_FILE=$CLUSTER_DIR/.pids

build() {
  echo "Building ephemera..."
  cargo build --release
}

create_cluster() {
  build

  echo "Creating configuration for ${NR_OF_NODES} nodes..."
  for ((c = 1; c <= NR_OF_NODES; c++)); do
    $EPHEMERA init --name node"$c" --port 300"$c"
  done
  $EPHEMERA add-local-peers --ephemera-root-dir ~/.ephemera

  status=$?
  [ $status -eq 0 ] && echo "Successfully created cluster" || echo "Creating cluster failed"

}

run_signatures_app() {
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

  echo "Running ephemera signatures application instances in ${APP_DIR} ..."
  echo "Starting $COUNTER nodes"

  export RUST_LOG="ephemera=debug"

  COUNTER=1

  mkdir -p "$CLUSTER_DIR"
  mkdir -p "$CLUSTER_DIR"/logs
  mkdir -p "$CLUSTER_DIR"/signatures
  mkdir -p "$CLUSTER_DIR"/db
  touch "$PIDS_FILE"

  CLIENT_LISTENER_ADDR=127.0.0.1
  WS_LISTENER_ADDR=127.0.0.1
  SIGNATURES_FILE=$CLUSTER_DIR/signatures/signatures
  LOGS_FILE=$CLUSTER_DIR/logs/

  COUNTER=1
  for d in ~/.ephemera/*/ephemera.toml; do
    DB_FILE="$CLUSTER_DIR"/db/ephemera$COUNTER.sqlite
    touch "$DB_FILE"
    export export DATABASE_FILE=$DB_FILE
    export export DATABASE_URL=sqlite:$DB_FILE
    cargo build --manifest-path "$PROJECT_ROOT"/Cargo.toml --release

    echo "Starting $d"
    $SIGNATURES_APP --config-file ~/.ephemera/node"${COUNTER}"/ephemera.toml \
     --client-listener-address $CLIENT_LISTENER_ADDR:400"$COUNTER" --signatures-file "$SIGNATURES_FILE""$COUNTER".txt \
     --ws-listen-addr=$WS_LISTENER_ADDR:600"$COUNTER" --db-url="$DB_FILE"\
      > "$LOGS_FILE"node"$COUNTER".log 2>&1 &

    echo "$!" >> "$PIDS_FILE"
    COUNTER=$((COUNTER + 1))
  done

  echo "Started $((COUNTER - 1)) ephemera signatures application instances."
  echo "Log files are in $LOGS_FILE directory."
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
[[ $SUBCMD == "run" ]] && run_signatures_app
[[ $SUBCMD == "stop" ]] && stop_cluster
