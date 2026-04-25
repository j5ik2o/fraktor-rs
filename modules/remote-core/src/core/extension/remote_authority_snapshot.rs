//! Immutable snapshot describing a single remote authority.

use alloc::string::String;

use crate::core::address::Address;

/// Immutable snapshot describing a single remote authority at a point in
/// time.
///
/// This is the pure data representation handed out by the extension layer
/// (e.g. for observability panels). All fields are private; accessors are
/// `&self` only, matching Pekko's `RemoteAuthoritySnapshot` shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteAuthoritySnapshot {
  address:           Address,
  is_connected:      bool,
  is_quarantined:    bool,
  last_contact_ms:   Option<u64>,
  quarantine_reason: Option<String>,
}

impl RemoteAuthoritySnapshot {
  /// Creates a new [`RemoteAuthoritySnapshot`].
  #[must_use]
  pub const fn new(
    address: Address,
    is_connected: bool,
    is_quarantined: bool,
    last_contact_ms: Option<u64>,
    quarantine_reason: Option<String>,
  ) -> Self {
    Self { address, is_connected, is_quarantined, last_contact_ms, quarantine_reason }
  }

  /// Returns the authority address.
  #[must_use]
  pub const fn address(&self) -> &Address {
    &self.address
  }

  /// Returns `true` when the authority is currently connected.
  #[must_use]
  pub const fn is_connected(&self) -> bool {
    self.is_connected
  }

  /// Returns `true` when the authority is currently quarantined.
  #[must_use]
  pub const fn is_quarantined(&self) -> bool {
    self.is_quarantined
  }

  /// Returns the monotonic millis of the last contact, if any.
  #[must_use]
  pub const fn last_contact_ms(&self) -> Option<u64> {
    self.last_contact_ms
  }

  /// Returns the human-readable quarantine reason, if any.
  #[must_use]
  pub fn quarantine_reason(&self) -> Option<&str> {
    self.quarantine_reason.as_deref()
  }
}
