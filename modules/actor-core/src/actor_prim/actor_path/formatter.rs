//! Canonical URI formatter for actor paths.

use alloc::string::{String, ToString};

use super::ActorPath;

/// Formats an [`ActorPath`] into Pekko-compatible canonical URIs.
pub struct ActorPathFormatter;

impl ActorPathFormatter {
  #[must_use]
  /// Formats `path` as `pekko://system@host:port/...`.
  pub fn format(path: &ActorPath) -> String {
    let mut canonical = String::new();
    let parts = path.parts();
    canonical.push_str(parts.scheme().as_str());
    canonical.push_str("://");
    canonical.push_str(parts.system());
    if let Some(authority) = parts.authority()
      && !authority.host().is_empty()
    {
      canonical.push('@');
      canonical.push_str(authority.host());
      if let Some(port) = authority.port() {
        canonical.push(':');
        canonical.push_str(&port.to_string());
      }
    }
    let relative = path.to_relative_string();
    if relative.is_empty() {
      canonical.push('/');
    } else {
      canonical.push_str(&relative);
    }
    if let Some(uid) = path.uid() {
      canonical.push('#');
      canonical.push_str(&uid.value().to_string());
    }
    canonical
  }
}
