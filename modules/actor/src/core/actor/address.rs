//! Unified address representation for actor systems.

#[cfg(test)]
mod tests;

use alloc::string::String;

use super::actor_path::{ActorPathParts, ActorPathScheme};

/// Unified address representing an actor system endpoint.
///
/// This mirrors Pekko's `Address` type, combining protocol, system name, and
/// optional host/port into a single value. The protocol is a free-form string
/// following Pekko's design (`protocol: String`), enabling arbitrary transport
/// schemes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Address {
  protocol: String,
  system:   String,
  host:     Option<String>,
  port:     Option<u16>,
}

impl Address {
  /// Creates an address with the given protocol and system name (no host/port).
  ///
  /// This corresponds to Pekko's `Address(protocol, system)`.
  #[must_use]
  pub fn new(protocol: impl Into<String>, system: impl Into<String>) -> Self {
    Self { protocol: protocol.into(), system: system.into(), host: None, port: None }
  }

  /// Creates a remote address with the given protocol, system name, host, and port.
  ///
  /// This corresponds to Pekko's `Address(protocol, system, host, port)`.
  #[must_use]
  pub fn new_remote(
    protocol: impl Into<String>,
    system: impl Into<String>,
    host: impl Into<String>,
    port: u16,
  ) -> Self {
    Self { protocol: protocol.into(), system: system.into(), host: Some(host.into()), port: Some(port) }
  }

  /// Creates a local address using the default fraktor protocol.
  #[must_use]
  pub fn local(system: impl Into<String>) -> Self {
    Self::new(ActorPathScheme::Fraktor.as_str(), system)
  }

  /// Creates a remote address using the default fraktor TCP protocol.
  #[must_use]
  pub fn remote(system: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
    Self::new_remote(ActorPathScheme::FraktorTcp.as_str(), system, host, port)
  }

  /// Builds an address from [`ActorPathParts`].
  #[must_use]
  pub fn from_parts(parts: &ActorPathParts) -> Self {
    Self {
      protocol: String::from(parts.scheme().as_str()),
      system:   String::from(parts.system()),
      host:     parts.authority().map(|a| String::from(a.host())),
      port:     parts.authority().and_then(|a| a.port()),
    }
  }

  /// Returns the transport protocol.
  #[must_use]
  pub fn protocol(&self) -> &str {
    &self.protocol
  }

  /// Returns the logical actor system name.
  #[must_use]
  pub fn system(&self) -> &str {
    &self.system
  }

  /// Returns the host portion, if configured.
  #[must_use]
  pub fn host(&self) -> Option<&str> {
    self.host.as_deref()
  }

  /// Returns the port number, if configured.
  #[must_use]
  pub const fn port(&self) -> Option<u16> {
    self.port
  }

  /// Returns true when the address represents a remote endpoint.
  #[must_use]
  pub const fn has_global_scope(&self) -> bool {
    self.host.is_some()
  }

  /// Returns true when the address represents a local endpoint.
  #[must_use]
  pub const fn has_local_scope(&self) -> bool {
    self.host.is_none()
  }

  /// Returns the `system@host:port` portion (without the protocol scheme).
  ///
  /// This mirrors Pekko's `Address.hostPort`.
  #[must_use]
  pub fn host_port(&self) -> String {
    match (&self.host, self.port) {
      | (Some(host), Some(port)) => alloc::format!("{}@{}:{}", self.system, host, port),
      | (Some(host), None) => alloc::format!("{}@{}", self.system, host),
      | _ => self.system.clone(),
    }
  }

  /// Formats the address as a URI-like string.
  #[must_use]
  pub fn to_uri_string(&self) -> String {
    let scheme = &self.protocol;
    match (&self.host, self.port) {
      | (Some(host), Some(port)) => alloc::format!("{}://{}@{}:{}", scheme, self.system, host, port),
      | (Some(host), None) => alloc::format!("{}://{}@{}", scheme, self.system, host),
      | _ => alloc::format!("{}://{}", scheme, self.system),
    }
  }
}

impl core::fmt::Display for Address {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.write_str(&self.to_uri_string())
  }
}
