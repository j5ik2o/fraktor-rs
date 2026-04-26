//! Canonical remote address (`host + port + system`).

use alloc::string::String;
use core::fmt::{Display, Formatter, Result as FmtResult};

/// Canonical remote address identifying an actor system endpoint.
///
/// Modeled after Apache Pekko's `Address`, but without the `protocol` field — the
/// scheme is expressed separately through [`crate::domain::address::ActorPathScheme`] when
/// a full URI is needed.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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

impl Display for Address {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(f, "{}@{}:{}", self.system, self.host, self.port)
  }
}
