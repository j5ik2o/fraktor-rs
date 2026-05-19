//! Group router configuration.
//!
//! Corresponds to Pekko's `org.apache.pekko.routing.Group`.

use alloc::vec::Vec;

use super::router_config::RouterConfig;

/// Configuration for a router that routes to externally created routees.
///
/// A group router does not create routees itself. Instead, it routes messages
/// to existing actors identified by their paths. Routees are discovered by the
/// caller and supplied as external actor paths.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.Group`.
pub trait Group: RouterConfig {
  /// Returns the actor paths of the routees.
  fn paths(&self) -> Vec<&str>;
}
