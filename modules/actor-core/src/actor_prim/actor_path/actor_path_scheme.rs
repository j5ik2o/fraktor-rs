//! Actor path URI scheme definitions.

/// Canonical scheme supported by the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActorPathScheme {
  /// Local Fraktor transport.
  Fraktor,
  /// TCP transport compatible with Fraktor remoting.
  FraktorTcp,
}

impl ActorPathScheme {
  #[must_use]
  /// Returns the canonical scheme string.
  pub const fn as_str(&self) -> &'static str {
    match self {
      | ActorPathScheme::Fraktor => "fraktor",
      | ActorPathScheme::FraktorTcp => "fraktor.tcp",
    }
  }
}
