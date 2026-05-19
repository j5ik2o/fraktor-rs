//! Custom router configuration base.
//!
//! Corresponds to Pekko's `org.apache.pekko.routing.CustomRouterConfig`.

use alloc::string::String;

use super::{Router, RoutingLogic, router_config::RouterConfig};

/// Base for custom router implementations that are neither [`Pool`](super::Pool)
/// nor [`Group`](super::Group).
///
/// Provides a default `router_dispatcher` (the default dispatcher) and
/// leaves `create_router` to the implementor.
///
/// Corresponds to Pekko's `org.apache.pekko.routing.CustomRouterConfig`.
pub trait CustomRouterConfig: Send + Sync {
  /// The routing logic type produced by this configuration.
  type Logic: RoutingLogic;

  /// Creates the [`Router`] instance for this custom configuration.
  fn create_router(&self) -> Router<Self::Logic>;
}

/// Blanket [`RouterConfig`] implementation for all [`CustomRouterConfig`] types.
///
/// Custom routers use the default dispatcher and always stop when all routees
/// are removed.
impl<T: CustomRouterConfig> RouterConfig for T {
  type Logic = T::Logic;

  fn create_router(&self) -> Router<Self::Logic> {
    CustomRouterConfig::create_router(self)
  }

  fn router_dispatcher(&self) -> String {
    String::from("default-dispatcher")
  }
}
