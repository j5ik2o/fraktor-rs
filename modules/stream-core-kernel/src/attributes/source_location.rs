use alloc::{borrow::Cow, boxed::Box, format, string::String};
use core::any::Any;

use super::Attribute;

/// Source callsite attribute used for diagnostics.
///
/// Mirrors Pekko's `Attributes.SourceLocation`. Pekko stores a
/// `lambda: AnyRef` and derives a location name from JVM bytecode
/// line-number metadata. Rust has no equivalent introspection, so this
/// translation captures the callsite components directly (file, line,
/// column) — matching the information produced by
/// [`core::panic::Location`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
  file:   Cow<'static, str>,
  line:   u32,
  column: u32,
}

impl SourceLocation {
  /// Creates a new `SourceLocation` from the supplied callsite components.
  #[must_use]
  pub const fn new(file: Cow<'static, str>, line: u32, column: u32) -> Self {
    Self { file, line, column }
  }

  /// Returns the source file component of the callsite.
  #[must_use]
  pub fn file(&self) -> &str {
    &self.file
  }

  /// Returns the source line number.
  #[must_use]
  pub const fn line(&self) -> u32 {
    self.line
  }

  /// Returns the source column number.
  #[must_use]
  pub const fn column(&self) -> u32 {
    self.column
  }

  /// Returns a `"file:line:column"` rendering of the callsite.
  ///
  /// Mirrors Pekko's `SourceLocation.locationName` in spirit while
  /// exposing the Rust-native callsite tuple rather than a lambda class
  /// name.
  #[must_use]
  pub fn location_name(&self) -> String {
    format!("{}:{}:{}", self.file, self.line, self.column)
  }
}

impl Attribute for SourceLocation {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn clone_box(&self) -> Box<dyn Attribute> {
    Box::new(self.clone())
  }

  fn eq_attr(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<Self>() == Some(self)
  }
}
