use crate::peer_discovery::PeerId;

pub(crate) mod behaviour;
mod handler;

const MAX_DIAL_ATTEMPT_ROUNDS: usize = 6;

/// Minimum percentage of available nodes to consider the network healthy.
//TODO: make this configurable
const MEMBERSHIP_MINIMUM_AVAILABLE_NODES_RATIO: f64 = 0.8;

/// Maximum percentage of nodes that can change in a single membership update.
/// In general it should be considered a security risk if it has changed too much.
/// //TODO: make this configurable
const MEMBERSHIP_MAXIMUM_ALLOWED_CHANGE_RATIO: f64 = 0.2;

pub(crate) fn calculate_membership_change(_previous: Vec<PeerId>, _current: Vec<PeerId>) -> usize {
    0
}