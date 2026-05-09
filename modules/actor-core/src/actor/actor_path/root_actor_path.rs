//! Type-safe wrapper guaranteeing a root actor path.

#[cfg(test)]
mod tests;

use core::fmt::{Debug, Display, Formatter, Result as FmtResult};

use super::{ActorPath, ActorPathError, ActorPathParts, GuardianKind};

/// A root actor path that has only the guardian segment.
///
/// In Pekko, `RootActorPath` represents the root of the actor hierarchy.
/// This wrapper guarantees at the type level that the inner [`ActorPath`]
/// contains exactly the guardian segment and no child segments.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RootActorPath {
  inner: ActorPath,
}

impl RootActorPath {
  /// Creates a root path for the default (user) guardian.
  #[must_use]
  pub fn new() -> Self {
    Self { inner: ActorPath::root() }
  }

  /// Creates a root path for the specified guardian kind.
  #[must_use]
  pub fn with_guardian(guardian: GuardianKind) -> Self {
    Self { inner: ActorPath::root_with_guardian(guardian) }
  }

  /// Creates a root path from explicit parts.
  ///
  /// # Errors
  ///
  /// Returns [`ActorPathError::NotRootPath`] if the resulting path has child segments.
  pub fn from_parts(parts: ActorPathParts) -> Result<Self, ActorPathError> {
    let path = ActorPath::from_parts(parts);
    Self::try_from_path(path)
  }

  /// Attempts to interpret a generic [`ActorPath`] as a root path.
  ///
  /// A root path has exactly one segment: the guardian segment.
  ///
  /// # Errors
  ///
  /// Returns [`ActorPathError::NotRootPath`] if the path has child segments.
  pub fn try_from_path(path: ActorPath) -> Result<Self, ActorPathError> {
    if path.segments().len() == 1 { Ok(Self { inner: path }) } else { Err(ActorPathError::NotRootPath) }
  }

  /// Returns the inner [`ActorPath`].
  #[must_use]
  pub fn into_inner(self) -> ActorPath {
    self.inner
  }

  /// Returns a reference to the inner [`ActorPath`].
  #[must_use]
  pub const fn as_path(&self) -> &ActorPath {
    &self.inner
  }

  /// Returns path parts metadata.
  #[must_use]
  pub const fn parts(&self) -> &ActorPathParts {
    self.inner.parts()
  }

  /// Returns the guardian kind of this root path.
  #[must_use]
  pub const fn guardian(&self) -> GuardianKind {
    self.inner.guardian()
  }
}

impl Default for RootActorPath {
  fn default() -> Self {
    Self::new()
  }
}

impl Display for RootActorPath {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    Display::fmt(&self.inner, f)
  }
}

impl Debug for RootActorPath {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_tuple("RootActorPath").field(&self.inner.to_relative_string()).finish()
  }
}

impl From<RootActorPath> for ActorPath {
  fn from(root: RootActorPath) -> Self {
    root.inner
  }
}
