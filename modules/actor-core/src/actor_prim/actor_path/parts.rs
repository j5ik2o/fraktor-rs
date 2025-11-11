//! Metadata components that describe canonical actor paths.

use alloc::string::String;

/// Canonical scheme supported by the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActorPathScheme {
  /// Local Pekko transport.
  Pekko,
  /// TCP transport compatible with Pekko remoting.
  PekkoTcp,
}

impl ActorPathScheme {
  #[must_use]
  /// Returns the canonical scheme string.
  pub fn as_str(&self) -> &'static str {
    match self {
      | ActorPathScheme::Pekko => "pekko",
      | ActorPathScheme::PekkoTcp => "pekko.tcp",
    }
  }
}

/// Guardian hierarchy that anchors the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuardianKind {
  /// `/system` guardian target.
  System,
  /// `/user` guardian target.
  User,
}

impl GuardianKind {
  #[must_use]
  /// Returns the textual guardian segment.
  pub fn segment(&self) -> &'static str {
    match self {
      | GuardianKind::System => "system",
      | GuardianKind::User => "user",
    }
  }
}

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
  pub(crate) fn port(&self) -> Option<u16> {
    self.port
  }
}

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
    Self { scheme: ActorPathScheme::Pekko, system: system.into(), authority: None, guardian: GuardianKind::User }
  }

  #[must_use]
  /// Overrides the URI scheme.
  pub fn with_scheme(mut self, scheme: ActorPathScheme) -> Self {
    self.scheme = scheme;
    self
  }

  #[must_use]
  /// Overrides the guardian segment inserted at the beginning of the path.
  pub fn with_guardian(mut self, guardian: GuardianKind) -> Self {
    self.guardian = guardian;
    self
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
  pub fn scheme(&self) -> ActorPathScheme {
    self.scheme
  }

  #[must_use]
  /// Returns the logical actor system name.
  pub fn system(&self) -> &str {
    &self.system
  }

  #[must_use]
  /// Returns the guardian kind anchoring the path.
  pub fn guardian(&self) -> GuardianKind {
    self.guardian
  }

  #[must_use]
  /// Returns the guardian segment string.
  pub fn guardian_segment(&self) -> &'static str {
    self.guardian.segment()
  }

  #[must_use]
  pub(crate) fn authority(&self) -> Option<&PathAuthority> {
    self.authority.as_ref()
  }
}
