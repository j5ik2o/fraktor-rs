//! Compression table advertisement metadata.

use alloc::vec::Vec;

use super::{CompressionTableEntry, CompressionTableKind};

/// Compression table entries advertised for one table kind and generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompressionAdvertisement {
  table_kind: CompressionTableKind,
  generation: u64,
  entries:    Vec<CompressionTableEntry>,
}

impl CompressionAdvertisement {
  /// Creates a compression table advertisement.
  #[must_use]
  pub const fn new(table_kind: CompressionTableKind, generation: u64, entries: Vec<CompressionTableEntry>) -> Self {
    Self { table_kind, generation, entries }
  }

  /// Returns the advertised table kind.
  #[must_use]
  pub const fn table_kind(&self) -> CompressionTableKind {
    self.table_kind
  }

  /// Returns the advertisement generation.
  #[must_use]
  pub const fn generation(&self) -> u64 {
    self.generation
  }

  /// Returns the advertised table entries.
  #[must_use]
  pub const fn entries(&self) -> &[CompressionTableEntry] {
    self.entries.as_slice()
  }

  /// Consumes this advertisement and returns its entries.
  #[must_use]
  pub fn into_entries(self) -> Vec<CompressionTableEntry> {
    self.entries
  }
}
