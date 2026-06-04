//! Input for `Send` path target selection.

use super::{MediatorPathKey, PubSubEnvelope};

/// Input used to select one path delivery target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendPathInput {
  /// Canonical address-less path key.
  pub path:           MediatorPathKey,
  /// Serialized payload.
  pub payload:        PubSubEnvelope,
  /// Whether local owner entries should be preferred.
  pub local_affinity: bool,
}

impl SendPathInput {
  /// Creates a send path input.
  #[must_use]
  pub const fn new(path: MediatorPathKey, payload: PubSubEnvelope, local_affinity: bool) -> Self {
    Self { path, payload, local_affinity }
  }
}
