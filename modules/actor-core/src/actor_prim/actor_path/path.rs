//! In-memory representation of canonical actor paths.

use alloc::{
  string::{String, ToString},
  vec::Vec,
};
use core::fmt;

use super::{
  ActorPathError, formatter::ActorPathFormatter, parts::ActorPathParts, segment::PathSegment, uid::ActorUid,
};

/// Canonical actor path with scheme/system/authority metadata.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ActorPath {
  parts:    ActorPathParts,
  segments: Vec<PathSegment>,
  uid:      Option<ActorUid>,
}

impl ActorPath {
  /// Creates an empty path using default local parts.
  #[must_use]
  pub fn root() -> Self {
    Self::empty(ActorPathParts::local("cellactor"))
  }

  /// Builds a path from explicit parts, automatically injecting guardian segments.
  #[must_use]
  pub fn from_parts(parts: ActorPathParts) -> Self {
    let mut path = Self::empty(parts.clone());
    path.push_guardian();
    path
  }

  /// Builds a path from explicit parts and segments without auto-injecting guardian.
  #[must_use]
  pub(crate) fn from_parts_and_segments(
    parts: ActorPathParts,
    segments: Vec<PathSegment>,
    uid: Option<ActorUid>,
  ) -> Self {
    Self { parts, segments, uid }
  }

  fn push_guardian(&mut self) {
    let guardian = self.parts.guardian_segment();
    // Guardian names never contain reserved characters.
    let segment = PathSegment::new(guardian).expect("guardian segment must be valid");
    if !self.segments.is_empty() {
      return;
    }
    self.segments.push(segment);
  }

  fn empty(parts: ActorPathParts) -> Self {
    Self { parts, segments: Vec::new(), uid: None }
  }

  /// Attempts to build a path from raw segments, validating each component.
  pub fn try_from_segments<I, S>(segments: I) -> Result<Self, ActorPathError>
  where
    I: IntoIterator<Item = S>,
    S: Into<String>, {
    let mut path = Self::empty(ActorPathParts::local("cellactor"));
    for segment in segments.into_iter() {
      path.segments.push(PathSegment::new(segment.into())?);
    }
    Ok(path)
  }

  /// Builds a path from segments, panicking on invalid data (legacy API).
  #[must_use]
  pub fn from_segments<I, S>(segments: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>, {
    Self::try_from_segments(segments).expect("invalid actor path segment")
  }

  /// Returns path parts metadata.
  #[must_use]
  pub fn parts(&self) -> &ActorPathParts {
    &self.parts
  }

  /// Returns UID if present.
  #[must_use]
  pub fn uid(&self) -> Option<ActorUid> {
    self.uid
  }

  /// Sets the UID, returning a new path.
  #[must_use]
  pub fn with_uid(mut self, uid: ActorUid) -> Self {
    self.uid = Some(uid);
    self
  }

  /// Returns validated segments.
  #[must_use]
  pub fn segments(&self) -> &[PathSegment] {
    &self.segments
  }

  /// Appends a child segment, panicking on invalid names.
  #[must_use]
  pub fn child(&self, name: impl AsRef<str>) -> Self {
    self.try_child(name.as_ref()).expect("invalid child segment")
  }

  /// Fallible variant that validates child segments.
  pub fn try_child(&self, name: &str) -> Result<Self, ActorPathError> {
    let mut clone = self.clone();
    clone.segments.push(PathSegment::new(name.to_string())?);
    Ok(clone)
  }

  /// Converts the relative path (`/system/user/worker`) into a string.
  #[must_use]
  pub fn to_relative_string(&self) -> String {
    if self.segments.is_empty() {
      return "/".into();
    }
    let mut value = String::new();
    for segment in &self.segments {
      value.push('/');
      value.push_str(segment.as_str());
    }
    value
  }

  /// Backwards-compatible helper that forwards to [`to_relative_string`].
  #[must_use]
  pub fn to_string(&self) -> String {
    self.to_relative_string()
  }

  /// Formats the path as `pekko://system@host:port/...`.
  #[must_use]
  pub fn to_canonical_uri(&self) -> String {
    ActorPathFormatter::format(self)
  }
}

impl fmt::Display for ActorPath {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.to_relative_string())
  }
}

impl fmt::Debug for ActorPath {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_tuple("ActorPath").field(&self.to_relative_string()).finish()
  }
}
