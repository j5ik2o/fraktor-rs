//! Input for `SendToAll` path target selection.

use super::{MediatorPathKey, PubSubEnvelope};

/// Input used to select all matching path delivery targets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendToAllPathInput {
  /// Canonical address-less path key.
  pub path:         MediatorPathKey,
  /// Serialized payload.
  pub payload:      PubSubEnvelope,
  /// Whether local owner entries should be excluded.
  pub all_but_self: bool,
}

impl SendToAllPathInput {
  /// Creates a send-to-all path input.
  #[must_use]
  pub const fn new(path: MediatorPathKey, payload: PubSubEnvelope, all_but_self: bool) -> Self {
    Self { path, payload, all_but_self }
  }
}
