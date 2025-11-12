//! Authority settings for canonical URIs.

use alloc::string::String;

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
}
