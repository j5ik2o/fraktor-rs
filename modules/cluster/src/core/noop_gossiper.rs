//! No-op implementation of the Gossiper trait.

use crate::core::gossiper::Gossiper;

/// A no-op gossiper that does nothing.
///
/// This implementation is useful for testing, single-node clusters,
/// or static topology scenarios where gossip is not required.
#[derive(Clone, Debug, Default)]
pub struct NoopGossiper;

impl NoopGossiper {
  /// Creates a new no-op gossiper.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl Gossiper for NoopGossiper {
  fn start(&mut self) -> Result<(), &'static str> {
    Ok(())
  }

  fn stop(&mut self) -> Result<(), &'static str> {
    Ok(())
  }
}
