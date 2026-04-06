//! Immutable metadata snapshot for [`TypedActorSystem`].

use alloc::string::String;
use core::time::Duration;

/// Immutable settings snapshot exposed by [`TypedActorSystem::settings`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedActorSystemSettings {
  system_name: String,
  start_time:  Duration,
}

impl TypedActorSystemSettings {
  /// Creates a new settings snapshot.
  #[must_use]
  pub(crate) const fn new(system_name: String, start_time: Duration) -> Self {
    Self { system_name, start_time }
  }

  /// Returns the configured actor system name.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // String の Deref が const でないため const fn にできない
  pub fn system_name(&self) -> &str {
    &self.system_name
  }

  /// Returns the configured actor system start time.
  #[must_use]
  pub const fn start_time(&self) -> Duration {
    self.start_time
  }
}
