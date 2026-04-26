//! Handshake request body.

use alloc::string::String;

/// Body of a handshake request carrying the origin node identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandshakeReq {
  origin_system: String,
  origin_host:   String,
  origin_port:   u16,
  origin_uid:    u64,
}

impl HandshakeReq {
  /// Creates a new [`HandshakeReq`].
  #[must_use]
  pub const fn new(origin_system: String, origin_host: String, origin_port: u16, origin_uid: u64) -> Self {
    Self { origin_system, origin_host, origin_port, origin_uid }
  }

  /// Returns the origin actor system name.
  #[must_use]
  pub fn origin_system(&self) -> &str {
    &self.origin_system
  }

  /// Returns the origin host name.
  #[must_use]
  pub fn origin_host(&self) -> &str {
    &self.origin_host
  }

  /// Returns the origin port.
  #[must_use]
  pub const fn origin_port(&self) -> u16 {
    self.origin_port
  }

  /// Returns the origin unique id.
  #[must_use]
  pub const fn origin_uid(&self) -> u64 {
    self.origin_uid
  }
}
