//! Canonical remote address (`host + port + system`).

use alloc::string::String;
use core::{
  fmt,
  hash::{Hash, Hasher},
};

/// Canonical remote address identifying an actor system endpoint.
///
/// Modeled after Apache Pekko's `Address`, but without the `protocol` field — the
/// scheme is expressed separately through [`crate::address::ActorPathScheme`] when
/// a full URI is needed.
#[derive(Clone, Debug)]
pub struct Address {
  host:   String,
  port:   u16,
  system: String,
}

impl Address {
  /// Creates a new [`Address`].
  #[must_use]
  pub fn new(system: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
    Self { host: host.into(), port, system: system.into() }
  }

  /// Returns the actor system name.
  #[must_use]
  pub fn system(&self) -> &str {
    &self.system
  }

  /// Returns the host name.
  #[must_use]
  pub fn host(&self) -> &str {
    &self.host
  }

  /// Returns the port.
  #[must_use]
  pub const fn port(&self) -> u16 {
    self.port
  }
}

impl PartialEq for Address {
  fn eq(&self, other: &Self) -> bool {
    self.port == other.port && self.system == other.system && self.host == other.host
  }
}

impl Eq for Address {}

impl Hash for Address {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.system.hash(state);
    self.host.hash(state);
    self.port.hash(state);
  }
}

impl fmt::Display for Address {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}@{}:{}", self.system, self.host, self.port)
  }
}
