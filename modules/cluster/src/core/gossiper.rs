//! Abstraction over gossip lifecycle.

/// Drives gossip start/stop around membership dissemination.
pub trait Gossiper: Send + Sync {
  /// Starts gossip dissemination.
  fn start(&self) -> Result<(), &'static str>;

  /// Stops gossip dissemination.
  fn stop(&self) -> Result<(), &'static str>;
}
