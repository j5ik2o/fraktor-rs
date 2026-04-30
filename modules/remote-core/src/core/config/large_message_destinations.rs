//! Collection of large-message destination patterns.

use alloc::vec::Vec;

use crate::core::config::LargeMessageDestinationPattern;

/// Owned `no_std` collection of destination patterns that opt actors into the
/// dedicated large-message path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LargeMessageDestinations {
  patterns: Vec<LargeMessageDestinationPattern>,
}

impl LargeMessageDestinations {
  /// Creates an empty destination set.
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// Creates a destination set from already-built patterns.
  #[must_use]
  pub const fn from_patterns(patterns: Vec<LargeMessageDestinationPattern>) -> Self {
    Self { patterns }
  }

  /// Appends one path pattern.
  #[must_use]
  pub fn with_pattern(mut self, pattern: LargeMessageDestinationPattern) -> Self {
    self.patterns.push(pattern);
    self
  }

  /// Returns all configured patterns.
  #[must_use]
  pub fn patterns(&self) -> &[LargeMessageDestinationPattern] {
    &self.patterns
  }

  /// Returns the number of configured patterns.
  #[must_use]
  pub const fn len(&self) -> usize {
    self.patterns.len()
  }

  /// Returns `true` when no destination pattern is configured.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.patterns.is_empty()
  }

  /// Returns `true` when `path` matches at least one configured pattern.
  ///
  /// # Panics
  ///
  /// Panics when `path` is not an absolute actor path.
  #[must_use]
  pub fn matches_absolute_path(&self, path: &str) -> bool {
    self.patterns.iter().any(|pattern| pattern.matches_absolute_path(path))
  }
}
