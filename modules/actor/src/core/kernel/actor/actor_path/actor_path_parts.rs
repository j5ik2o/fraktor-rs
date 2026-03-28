//! Immutable actor path metadata components.

use alloc::string::String;

use super::{ActorPathScheme, GuardianKind, PathAuthority};

/// Immutable parts shared by ActorPath instances.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActorPathParts {
  scheme:    ActorPathScheme,
  system:    String,
  authority: Option<PathAuthority>,
  guardian:  GuardianKind,
}

impl ActorPathParts {
  #[must_use]
  /// Builds parts for a local (non-remote) actor system.
  pub fn local(system: impl Into<String>) -> Self {
    Self {
      scheme:    ActorPathScheme::Fraktor,
      system:    system.into(),
      authority: None,
      guardian:  GuardianKind::User,
    }
  }

  #[must_use]
  /// Overrides the URI scheme.
  pub const fn with_scheme(mut self, scheme: ActorPathScheme) -> Self {
    self.scheme = scheme;
    self
  }

  #[must_use]
  /// Overrides the guardian segment inserted at the beginning of the path.
  pub const fn with_guardian(mut self, guardian: GuardianKind) -> Self {
    self.guardian = guardian;
    self
  }

  #[must_use]
  /// Builds parts with a remote authority (host, port).
  pub fn with_authority(system: impl Into<String>, authority: Option<(impl Into<String>, u16)>) -> Self {
    let system = system.into();
    let authority = authority.map(|(host, port)| PathAuthority { host: host.into(), port: Some(port) });
    Self { scheme: ActorPathScheme::FraktorTcp, system, authority, guardian: GuardianKind::User }
  }

  #[must_use]
  /// Sets the authority host portion.
  pub fn with_authority_host(mut self, host: String) -> Self {
    match &mut self.authority {
      | Some(authority) => authority.host = host,
      | None => self.authority = Some(PathAuthority { host, port: None }),
    }
    self
  }

  #[must_use]
  /// Sets the authority port portion.
  pub fn with_authority_port(mut self, port: u16) -> Self {
    match &mut self.authority {
      | Some(authority) => authority.port = Some(port),
      | None => {
        self.authority = Some(PathAuthority { host: String::new(), port: Some(port) });
      },
    }
    self
  }

  #[must_use]
  /// Returns the configured scheme.
  pub const fn scheme(&self) -> ActorPathScheme {
    self.scheme
  }

  #[must_use]
  /// Returns the logical actor system name.
  #[allow(clippy::missing_const_for_fn)] // String の Deref が const でないため const fn にできない
  pub fn system(&self) -> &str {
    &self.system
  }

  #[must_use]
  /// Returns the guardian kind anchoring the path.
  pub const fn guardian(&self) -> GuardianKind {
    self.guardian
  }

  #[must_use]
  /// Returns the guardian segment string.
  pub const fn guardian_segment(&self) -> &'static str {
    self.guardian.segment()
  }

  /// Formats the authority (`host[:port]`) when present.
  #[must_use]
  pub fn authority_endpoint(&self) -> Option<String> {
    self.authority.as_ref().map(|authority| authority.endpoint())
  }

  #[must_use]
  pub(crate) const fn authority(&self) -> Option<&PathAuthority> {
    self.authority.as_ref()
  }
}
