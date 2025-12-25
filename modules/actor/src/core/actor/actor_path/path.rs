//! In-memory representation of canonical actor paths.

use alloc::{
  string::{String, ToString},
  vec::Vec,
};
use core::fmt;

use super::{ActorPathError, ActorPathParts, ActorUid, GuardianKind, PathSegment, formatter::ActorPathFormatter};

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
    Self::root_with_guardian(GuardianKind::User)
  }

  /// Creates an empty path anchored to the specified guardian.
  #[must_use]
  pub fn root_with_guardian(guardian: GuardianKind) -> Self {
    Self::from_parts(ActorPathParts::local("cellactor").with_guardian(guardian))
  }

  /// Builds a path from explicit parts, automatically injecting guardian segments.
  #[must_use]
  pub fn from_parts(parts: ActorPathParts) -> Self {
    Self::from_parts_and_segments(parts, Vec::new(), None)
  }

  /// Builds a path from explicit parts and segments without auto-injecting guardian.
  #[must_use]
  pub(crate) fn from_parts_and_segments(
    parts: ActorPathParts,
    segments: Vec<PathSegment>,
    uid: Option<ActorUid>,
  ) -> Self {
    let mut segments = segments;
    Self::ensure_guardian_prefix(&parts, &mut segments);
    Self { parts, segments, uid }
  }

  /// Attempts to build a path from raw segments, validating each component.
  ///
  /// # Errors
  ///
  /// Returns [`ActorPathError`] if any segment is invalid.
  pub fn try_from_segments<I, S>(segments: I) -> Result<Self, ActorPathError>
  where
    I: IntoIterator<Item = S>,
    S: Into<String>, {
    let mut validated = Vec::new();
    for segment in segments.into_iter() {
      validated.push(PathSegment::new(segment.into())?);
    }
    Ok(Self::from_parts_and_segments(ActorPathParts::local("cellactor"), validated, None))
  }

  /// Builds a path from segments, panicking on invalid data (legacy API).
  ///
  /// # Panics
  ///
  /// Panics if any segment is invalid.
  #[must_use]
  #[allow(clippy::expect_used)]
  pub fn from_segments<I, S>(segments: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>, {
    Self::try_from_segments(segments).expect("invalid actor path segment")
  }

  /// Returns path parts metadata.
  #[must_use]
  pub const fn parts(&self) -> &ActorPathParts {
    &self.parts
  }

  /// Returns UID if present.
  #[must_use]
  pub const fn uid(&self) -> Option<ActorUid> {
    self.uid
  }

  /// Sets the UID, returning a new path.
  #[must_use]
  pub const fn with_uid(mut self, uid: ActorUid) -> Self {
    self.uid = Some(uid);
    self
  }

  /// Returns validated segments.
  #[must_use]
  #[allow(clippy::missing_const_for_fn)] // Vec の Deref が const でないため const fn にできない
  pub fn segments(&self) -> &[PathSegment] {
    &self.segments
  }

  /// Appends a child segment, panicking on invalid names.
  ///
  /// # Panics
  ///
  /// Panics if the segment is invalid.
  #[must_use]
  #[allow(clippy::expect_used)]
  pub fn child(&self, name: impl AsRef<str>) -> Self {
    self.try_child(name.as_ref()).expect("invalid child segment")
  }

  /// Fallible variant that validates child segments.
  ///
  /// # Errors
  ///
  /// Returns [`ActorPathError`] if the segment is invalid.
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
  #[allow(clippy::inherent_to_string_shadow_display)]
  pub fn to_string(&self) -> String {
    self.to_relative_string()
  }

  /// Formats the path as `fraktor://system@host:port/...`.
  #[must_use]
  pub fn to_canonical_uri(&self) -> String {
    ActorPathFormatter::format(self)
  }

  fn ensure_guardian_prefix(parts: &ActorPathParts, segments: &mut Vec<PathSegment>) {
    let guardian = parts.guardian_segment();
    let needs_injection = segments.first().map(PathSegment::as_str) != Some(guardian);
    if needs_injection {
      #[allow(clippy::expect_used)]
      let segment = PathSegment::new(guardian).expect("guardian segment must be valid");
      segments.insert(0, segment);
    }
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
