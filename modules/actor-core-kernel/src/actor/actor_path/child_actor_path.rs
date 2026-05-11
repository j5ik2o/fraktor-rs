//! Type-safe wrapper guaranteeing a child actor path.

#[cfg(test)]
#[path = "child_actor_path_test.rs"]
mod tests;

use alloc::string::String;
use core::fmt::{Debug, Display, Formatter, Result as FmtResult};

use super::{ActorPath, ActorPathError, ActorPathParts, PathSegment};

/// A child actor path that has at least one segment beyond the guardian.
///
/// In Pekko, `ChildActorPath` represents any path below the root in the
/// actor hierarchy. This wrapper guarantees at the type level that the inner
/// [`ActorPath`] contains the guardian segment plus one or more child segments.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ChildActorPath {
  inner: ActorPath,
}

impl ChildActorPath {
  /// Attempts to interpret a generic [`ActorPath`] as a child path.
  ///
  /// A child path has more than one segment (guardian + at least one child).
  ///
  /// # Errors
  ///
  /// Returns [`ActorPathError::NotChildPath`] if the path has no child segments.
  pub fn try_from_path(path: ActorPath) -> Result<Self, ActorPathError> {
    if path.segments().len() >= 2 { Ok(Self { inner: path }) } else { Err(ActorPathError::NotChildPath) }
  }

  /// Creates a child path by appending a segment to the given parent path.
  ///
  /// # Errors
  ///
  /// Returns [`ActorPathError`] if the segment name is invalid.
  pub fn from_parent(parent: &ActorPath, name: &str) -> Result<Self, ActorPathError> {
    let child_path = parent.try_child(name)?;
    Ok(Self { inner: child_path })
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

  /// Returns the name of this child (the last segment).
  ///
  /// # Panics
  ///
  /// Panics if segments are empty, which should never happen as the
  /// constructor guarantees at least two segments.
  #[must_use]
  pub fn name(&self) -> &str {
    #[allow(clippy::expect_used)]
    self.inner.segments().last().expect("child path must have segments").as_str()
  }

  /// Returns all segments (including guardian).
  #[must_use]
  pub fn segments(&self) -> &[PathSegment] {
    self.inner.segments()
  }

  /// Creates a deeper child path by appending a segment.
  ///
  /// # Errors
  ///
  /// Returns [`ActorPathError`] if the segment name is invalid.
  pub fn try_child(&self, name: &str) -> Result<Self, ActorPathError> {
    let deeper = self.inner.try_child(name)?;
    Ok(Self { inner: deeper })
  }

  /// Converts the relative path into a string.
  #[must_use]
  pub fn to_relative_string(&self) -> String {
    self.inner.to_relative_string()
  }

  /// Formats the path as a canonical URI.
  #[must_use]
  pub fn to_canonical_uri(&self) -> String {
    self.inner.to_canonical_uri()
  }
}

impl Display for ChildActorPath {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    Display::fmt(&self.inner, f)
  }
}

impl Debug for ChildActorPath {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_tuple("ChildActorPath").field(&self.inner.to_relative_string()).finish()
  }
}

impl TryFrom<ActorPath> for ChildActorPath {
  type Error = ActorPathError;

  fn try_from(path: ActorPath) -> Result<Self, Self::Error> {
    Self::try_from_path(path)
  }
}

impl From<ChildActorPath> for ActorPath {
  fn from(child: ChildActorPath) -> Self {
    child.inner
  }
}
