//! Remote address termination event payload.

use alloc::string::String;

/// Event payload describing that a remote authority is considered terminated.
#[derive(Clone, Debug)]
pub struct AddressTerminatedEvent {
  authority:          String,
  reason:             String,
  observed_at_millis: u64,
}

impl AddressTerminatedEvent {
  /// Creates a new address termination event.
  #[must_use]
  pub fn new(authority: impl Into<String>, reason: impl Into<String>, observed_at_millis: u64) -> Self {
    Self { authority: authority.into(), reason: reason.into(), observed_at_millis }
  }

  /// Returns the terminated remote authority.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // String の Deref が const でないため const fn にできない
  pub fn authority(&self) -> &str {
    &self.authority
  }

  /// Returns the termination reason metadata.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // String の Deref が const でないため const fn にできない
  pub fn reason(&self) -> &str {
    &self.reason
  }

  /// Returns the monotonic millis timestamp at which the termination was observed.
  #[must_use]
  pub const fn observed_at_millis(&self) -> u64 {
    self.observed_at_millis
  }
}
