//! Logical actor path representation rooted at `/`.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};
use core::fmt;

/// Represents the hierarchical path of an actor within the system.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ActorPath {
  segments: Vec<String>,
}

impl ActorPath {
  /// Creates the root (`/`) path.
  #[must_use]
  pub const fn root() -> Self {
    Self { segments: Vec::new() }
  }

  /// Builds a path from the provided segments.
  ///
  /// # Panics
  ///
  /// Panics when `segments` contains an empty component.
  #[must_use]
  pub fn from_segments<I, S>(segments: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>, {
    let collected: Vec<String> = segments.into_iter().map(Into::into).collect();
    for segment in &collected {
      assert!(!segment.is_empty(), "path segment must not be empty");
    }
    Self { segments: collected }
  }

  /// Returns the stored segments.
  #[must_use]
  pub fn segments(&self) -> &[String] {
    &self.segments
  }

  /// Returns a new path with the provided child appended.
  ///
  /// # Panics
  ///
  /// Panics if the provided segment is empty.
  #[must_use]
  pub fn child(&self, name: impl Into<String>) -> Self {
    let mut segments = self.segments.clone();
    let segment = name.into();
    assert!(!segment.is_empty(), "child segment must not be empty");
    segments.push(segment);
    Self { segments }
  }

  /// Converts the path into a string (e.g. `/user/worker`).
  #[must_use]
  #[allow(clippy::inherent_to_string_shadow_display)]
  pub fn to_string(&self) -> String {
    if self.segments.is_empty() {
      return "/".into();
    }
    let mut value = String::with_capacity(self.estimate_len());
    for segment in &self.segments {
      value.push('/');
      value.push_str(segment);
    }
    value
  }

  fn estimate_len(&self) -> usize {
    self.segments.iter().map(String::len).sum::<usize>() + self.segments.len()
  }
}

impl fmt::Display for ActorPath {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.to_string())
  }
}

impl fmt::Debug for ActorPath {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_tuple("ActorPath").field(&self.to_string()).finish()
  }
}
