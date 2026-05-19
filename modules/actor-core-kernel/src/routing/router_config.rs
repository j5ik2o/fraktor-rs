//! Configuration trait for router actors.
//!
//! Corresponds to Pekko's `org.apache.pekko.routing.RouterConfig`.

use alloc::string::String;

use super::{Router, RoutingLogic};

/// Defines how a router is constructed and configured.
///
/// This is the kernel-layer abstraction that typed-layer router builders
/// (e.g. `PoolRouter`, `GroupRouter`) can implement to express their routing
/// configuration in a uniform way.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.RouterConfig`.
pub trait RouterConfig: Send + Sync {
  /// The routing logic type produced by this configuration.
  type Logic: RoutingLogic;

  /// Creates the [`Router`] instance that performs the actual message routing.
  fn create_router(&self) -> Router<Self::Logic>;

  /// Dispatcher ID to use for running the router head actor.
  ///
  /// Returns the default dispatcher ID when the configuration does not
  /// override it.
  fn router_dispatcher(&self) -> String;

  /// Whether the router should stop itself when all routees have been
  /// removed.
  ///
  /// Defaults to `true`.
  fn stop_router_when_all_routees_removed(&self) -> bool {
    true
  }
}
