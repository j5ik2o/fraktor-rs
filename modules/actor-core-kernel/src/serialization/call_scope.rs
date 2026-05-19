//! Serialization call scope enumeration.

/// Describes the context in which serialization occurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SerializationCallScope {
  /// Local in-memory messaging.
  Local,
  /// Remote transport where manifest discipline is required.
  Remote,
  /// Persistence storage scope.
  Persistence,
}
