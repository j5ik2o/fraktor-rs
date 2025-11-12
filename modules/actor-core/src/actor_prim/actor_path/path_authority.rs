//! Authority settings for canonical URIs.

use alloc::{format, string::String};

/// Authority settings (host/port) for canonical URIs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PathAuthority {
  pub(crate) host: String,
  pub(crate) port: Option<u16>,
}

impl PathAuthority {
  #[must_use]
  /// Returns the authority host if configured.
  pub(crate) fn host(&self) -> &str {
    &self.host
  }

  #[must_use]
  /// Returns the authority port.
  pub(crate) const fn port(&self) -> Option<u16> {
    self.port
  }

  #[must_use]
  /// Returns the formatted endpoint key (`host[:port]`).
  pub(crate) fn endpoint(&self) -> String {
    match self.port {
      | Some(port) => alloc::format!("{}:{}", self.host, port),
      | None => self.host.clone(),
    }
  }
}
