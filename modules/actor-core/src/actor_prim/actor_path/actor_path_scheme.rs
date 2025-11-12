//! Actor path URI scheme definitions.

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
  pub const fn as_str(&self) -> &'static str {
    match self {
      | ActorPathScheme::Pekko => "pekko",
      | ActorPathScheme::PekkoTcp => "pekko.tcp",
    }
  }
}
