use crate::peer::PeerId;

pub(crate) mod behaviour;
mod connections;
mod handler;
mod membership;
mod peers_requester;

const MAX_DIAL_ATTEMPT_ROUNDS: usize = 6;

/// Minimum percentage of available nodes to consider the network healthy.
//TODO: make this configurable
const MEMBERSHIP_MINIMUM_AVAILABLE_NODES_RATIO: f64 = 0.8;

/// Maximum percentage of nodes that can change in a single membership update.
/// In general it should be considered a security risk if it has changed too much.
/// //TODO: make this configurable
const _MEMBERSHIP_MAXIMUM_ALLOWED_CHANGE_RATIO: f64 = 0.2;

pub(crate) fn _calculate_membership_change(_previous: Vec<PeerId>, _current: Vec<PeerId>) -> usize {
    0
}
