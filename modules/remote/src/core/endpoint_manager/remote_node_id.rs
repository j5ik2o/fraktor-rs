//! Identifies a remote node participating in remoting.

use alloc::string::String;

/// Unique identifier describing a remote node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteNodeId {
  system: String,
  host:   String,
  port:   Option<u16>,
  uid:    u64,
}

impl RemoteNodeId {
  /// Creates a new identifier.
  #[must_use]
  pub fn new(system: impl Into<String>, host: impl Into<String>, port: Option<u16>, uid: u64) -> Self {
    Self { system: system.into(), host: host.into(), port, uid }
  }

  /// Returns the system name.
  #[must_use]
  pub fn system(&self) -> &str {
    &self.system
  }

  /// Returns the host component.
  #[must_use]
  pub fn host(&self) -> &str {
    &self.host
  }

  /// Returns the port component.
  #[must_use]
  pub const fn port(&self) -> Option<u16> {
    self.port
  }

  /// Returns the unique identifier.
  #[must_use]
  pub const fn uid(&self) -> u64 {
    self.uid
  }
}
