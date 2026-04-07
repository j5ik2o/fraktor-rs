//! Immutable settings bundle passed to dispatcher constructors.
//!
//! `DispatcherSettings` is the new dispatcher-side settings record introduced
//! by the dispatcher-pekko-1n-redesign change. It is intentionally smaller than
//! the legacy version: `schedule_adapter` is gone (the `ScheduleAdapter` family
//! has been removed), and `starvation_deadline` is dropped from the initial
//! version under YAGNI.

#[cfg(test)]
mod tests;

use alloc::string::String;
use core::{num::NonZeroUsize, time::Duration};

/// Immutable bundle of dispatcher tunables.
///
/// Cloned freely between configurators and concrete dispatcher constructors.
#[derive(Clone, Debug)]
pub struct DispatcherSettings {
  id:                  String,
  throughput:          NonZeroUsize,
  throughput_deadline: Option<Duration>,
  shutdown_timeout:    Duration,
}

impl DispatcherSettings {
  /// Creates a settings record with the supplied values.
  #[must_use]
  pub fn new(
    id: impl Into<String>,
    throughput: NonZeroUsize,
    throughput_deadline: Option<Duration>,
    shutdown_timeout: Duration,
  ) -> Self {
    Self { id: id.into(), throughput, throughput_deadline, shutdown_timeout }
  }

  /// Returns the dispatcher identifier.
  #[must_use]
  pub fn id(&self) -> &str {
    &self.id
  }

  /// Returns the configured throughput per drain pass.
  #[must_use]
  pub const fn throughput(&self) -> NonZeroUsize {
    self.throughput
  }

  /// Returns the optional throughput deadline.
  #[must_use]
  pub const fn throughput_deadline(&self) -> Option<Duration> {
    self.throughput_deadline
  }

  /// Returns the configured shutdown timeout for delayed dispatcher shutdown.
  #[must_use]
  pub const fn shutdown_timeout(&self) -> Duration {
    self.shutdown_timeout
  }

  /// Returns a clone of the dispatcher identifier as an owned string.
  #[must_use]
  pub fn id_owned(&self) -> String {
    self.id.clone()
  }

  /// Returns a copy with the supplied identifier.
  #[must_use]
  pub fn with_id(mut self, id: impl Into<String>) -> Self {
    self.id = id.into();
    self
  }

  /// Returns a copy with the supplied throughput.
  #[must_use]
  pub const fn with_throughput(mut self, throughput: NonZeroUsize) -> Self {
    self.throughput = throughput;
    self
  }

  /// Returns a copy with the supplied throughput deadline.
  #[must_use]
  pub const fn with_throughput_deadline(mut self, throughput_deadline: Option<Duration>) -> Self {
    self.throughput_deadline = throughput_deadline;
    self
  }

  /// Returns a copy with the supplied shutdown timeout.
  #[must_use]
  pub const fn with_shutdown_timeout(mut self, shutdown_timeout: Duration) -> Self {
    self.shutdown_timeout = shutdown_timeout;
    self
  }
}

impl DispatcherSettings {
  /// Returns a builder seeded with `id` and conservative defaults.
  ///
  /// Defaults are:
  /// - `throughput`: 5
  /// - `throughput_deadline`: `None`
  /// - `shutdown_timeout`: 1 second
  #[must_use]
  pub fn with_defaults(id: impl Into<String>) -> Self {
    // SAFETY: 5 is statically non-zero so `NonZeroUsize::new_unchecked` is sound.
    let throughput = unsafe { NonZeroUsize::new_unchecked(5) };
    Self::new(id, throughput, None, Duration::from_secs(1))
  }
}
