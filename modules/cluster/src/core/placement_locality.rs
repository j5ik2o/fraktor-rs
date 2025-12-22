//! Placement locality classification.

/// Indicates whether the placement is handled locally or remotely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementLocality {
  /// The current node is responsible for activation.
  Local,
  /// A remote node is responsible for activation.
  Remote,
}
