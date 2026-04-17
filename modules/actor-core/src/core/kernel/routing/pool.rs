//! Pool router configuration.
//!
//! Corresponds to Pekko's `org.apache.pekko.routing.Pool`.

use super::router_config::RouterConfig;

/// Configuration for a router that creates routees as child actors.
///
/// A pool router spawns and supervises its own routees. When a routee
/// terminates, the pool removes it from the routing table. The pool may
/// optionally use a resizer to adjust the number of routees dynamically.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.Pool`.
pub trait Pool: RouterConfig {
  /// Initial number of routee instances to spawn.
  fn nr_of_instances(&self) -> usize;

  /// Whether this pool has a dynamic resizer attached.
  ///
  /// When `true`,
  /// [`stop_router_when_all_routees_removed`](RouterConfig::stop_router_when_all_routees_removed)
  /// defaults to `false` (the resizer may create new routees later).
  fn has_resizer(&self) -> bool {
    false
  }

  /// Whether to use a dedicated dispatcher for the pool's routees.
  fn use_pool_dispatcher(&self) -> bool {
    false
  }
}
