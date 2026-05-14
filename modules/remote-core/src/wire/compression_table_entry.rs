//! Compression table entry metadata.

use alloc::string::String;

/// Advertised compression table entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompressionTableEntry {
  id:      u32,
  literal: String,
}

impl CompressionTableEntry {
  /// Creates a compression table entry.
  #[must_use]
  pub const fn new(id: u32, literal: String) -> Self {
    Self { id, literal }
  }

  /// Returns the stable entry id.
  #[must_use]
  pub const fn id(&self) -> u32 {
    self.id
  }

  /// Returns the literal value for this entry.
  #[must_use]
  pub const fn literal(&self) -> &str {
    self.literal.as_str()
  }
}
