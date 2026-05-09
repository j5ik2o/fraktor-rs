#[cfg(test)]
mod tests;

use alloc::string::String;

use crate::attributes::Attributes;

/// Diagnostic snapshot of a single stage logic inside an interpreter.
///
/// Corresponds to Pekko `LogicSnapshotImpl(index: Int, label: String, attributes: Attributes)`
/// returned by the graph interpreter when producing a `StreamSnapshot`.
#[derive(Debug, Clone)]
pub struct LogicSnapshot {
  index:      u32,
  label:      String,
  attributes: Attributes,
}

impl LogicSnapshot {
  /// Creates a new logic snapshot.
  ///
  /// The `label` parameter accepts both `&str` and owned `String` via
  /// `Into<String>` to match the ergonomic of Pekko's constructor.
  #[must_use]
  pub fn new(index: u32, label: impl Into<String>, attributes: Attributes) -> Self {
    Self { index, label: label.into(), attributes }
  }

  /// Returns the stage index within the enclosing interpreter.
  #[must_use]
  pub const fn index(&self) -> u32 {
    self.index
  }

  /// Returns the human-readable label of the stage.
  #[must_use]
  pub fn label(&self) -> &str {
    &self.label
  }

  /// Returns the attributes attached to the stage.
  #[must_use]
  pub const fn attributes(&self) -> &Attributes {
    &self.attributes
  }
}
