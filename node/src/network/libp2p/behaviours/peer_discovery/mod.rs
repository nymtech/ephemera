use crate::peer_discovery::PeerId;

pub(crate) mod behaviour;
mod handler;

const MAX_DIAL_ATTEMPT_ROUNDS: usize = 6;

/// Minimum percentage of available nodes to consider the network healthy.
//TODO: make this configurable
const TOPOLOGY_MINIMUM_AVAILABLE_NODES_RATIO: f64 = 0.8;

pub(crate) fn calculate_minimum_available_nodes(topology_size: usize) -> usize {
    (topology_size as f64 * TOPOLOGY_MINIMUM_AVAILABLE_NODES_RATIO) as usize
}

/// Maximum percentage of nodes that can change in a single topology update.
/// In general it should be considered a security risk if it has changed too much.
/// //TODO: make this configurable
const TOPOLOGY_MAXIMUM_ALLOWED_CHANGE_RATIO: f64 = 0.2;

pub(crate) fn calculate_topology_change(_previous: Vec<PeerId>, _current: Vec<PeerId>) -> usize {
    0
}
