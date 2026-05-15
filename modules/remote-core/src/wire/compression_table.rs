//! Compression table state for wire metadata.

#[cfg(test)]
#[path = "compression_table_test.rs"]
mod tests;

use alloc::{
  string::{String, ToString},
  vec::Vec,
};
use core::num::NonZeroUsize;

use super::{CompressedText, CompressionAdvertisement, CompressionTableEntry, CompressionTableKind};
use crate::wire::wire_error::WireError;

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompressionTableEntryState {
  id: u32,
  literal: String,
  hit_count: u64,
  advertised_generation: Option<u64>,
  acknowledged_generation: Option<u64>,
}

impl CompressionTableEntryState {
  const fn new(id: u32, literal: String) -> Self {
    Self { id, literal, hit_count: 0, advertised_generation: None, acknowledged_generation: None }
  }

  fn from_advertisement(entry: &CompressionTableEntry, generation: u64) -> Self {
    Self {
      id: entry.id(),
      literal: entry.literal().to_string(),
      hit_count: 0,
      advertised_generation: Some(generation),
      acknowledged_generation: Some(generation),
    }
  }
}

/// No-IO compression table state for a single peer and metadata kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompressionTable {
  max: Option<NonZeroUsize>,
  next_entry_id: u32,
  next_generation: u64,
  latest_pending_generation: Option<u64>,
  entries: Vec<CompressionTableEntryState>,
}

impl CompressionTable {
  /// Creates a compression table.
  #[must_use]
  pub const fn new(max: Option<NonZeroUsize>) -> Self {
    Self { max, next_entry_id: 1, next_generation: 1, latest_pending_generation: None, entries: Vec::new() }
  }

  /// Returns true when outbound compression for this table kind is enabled.
  #[must_use]
  pub const fn is_enabled(&self) -> bool {
    self.max.is_some()
  }

  /// Returns the configured advertisement size bound.
  #[must_use]
  pub const fn max(&self) -> Option<NonZeroUsize> {
    self.max
  }

  /// Returns the latest generation waiting for acknowledgement.
  #[must_use]
  pub const fn latest_pending_generation(&self) -> Option<u64> {
    self.latest_pending_generation
  }

  /// Returns the number of stored entries.
  #[must_use]
  pub const fn len(&self) -> usize {
    self.entries.len()
  }

  /// Returns true when the table has no stored entries.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// Returns the entry id for a literal, if the literal has been observed.
  #[must_use]
  pub fn entry_id(&self, literal: &str) -> Option<u32> {
    self.entries.iter().find(|entry| entry.literal == literal).map(|entry| entry.id)
  }

  /// Returns the hit count for a literal, if the literal has been observed.
  #[must_use]
  pub fn hit_count(&self, literal: &str) -> Option<u64> {
    self.entries.iter().find(|entry| entry.literal == literal).map(|entry| entry.hit_count)
  }

  /// Observes a literal value and updates its hit count.
  pub fn observe(&mut self, literal: &str) {
    let Some(max) = self.max else {
      return;
    };
    if let Some(entry) = self.entries.iter_mut().find(|entry| entry.literal == literal) {
      entry.hit_count = entry.hit_count.saturating_add(1);
      return;
    }
    if self.entries.len() >= max.get() {
      return;
    }
    let entry_id = self.next_entry_id;
    self.next_entry_id = self.next_entry_id.saturating_add(1);
    let mut entry = CompressionTableEntryState::new(entry_id, literal.to_string());
    entry.hit_count = 1;
    self.entries.push(entry);
  }

  /// Creates an advertisement for the highest-hit entries, bounded by the configured max.
  #[must_use]
  pub fn create_advertisement(&mut self, table_kind: CompressionTableKind) -> Option<CompressionAdvertisement> {
    let max = self.max?;
    if self.entries.is_empty() {
      return None;
    }
    if self.latest_pending_generation.is_some() {
      return None;
    }

    let generation = self.next_generation;
    self.next_generation = self.next_generation.saturating_add(1);
    self.latest_pending_generation = Some(generation);

    let mut indexes = (0..self.entries.len()).collect::<Vec<_>>();
    indexes.sort_by(|left, right| {
      let left_entry = &self.entries[*left];
      let right_entry = &self.entries[*right];
      right_entry.hit_count.cmp(&left_entry.hit_count).then_with(|| left_entry.id.cmp(&right_entry.id))
    });

    let mut entries = Vec::new();
    for index in indexes.into_iter().take(max.get()) {
      let entry = &mut self.entries[index];
      entry.advertised_generation = Some(generation);
      entries.push(CompressionTableEntry::new(entry.id, entry.literal.clone()));
    }

    Some(CompressionAdvertisement::new(table_kind, generation, entries))
  }

  /// Applies an acknowledgement for the latest pending generation.
  pub fn acknowledge(&mut self, generation: u64) -> bool {
    if self.latest_pending_generation != Some(generation) {
      return false;
    }

    let mut applied = false;
    for entry in &mut self.entries {
      if entry.advertised_generation == Some(generation) {
        entry.acknowledged_generation = Some(generation);
        applied = true;
      } else {
        entry.acknowledged_generation = None;
      }
    }
    if applied {
      self.latest_pending_generation = None;
    }
    applied
  }

  /// Encodes a literal using an acknowledged table reference when available.
  #[must_use]
  pub fn encode(&self, literal: &str) -> CompressedText {
    if !self.is_enabled() {
      return CompressedText::literal(literal.to_string());
    }
    self
      .entries
      .iter()
      .find(|entry| entry.literal == literal && entry.acknowledged_generation.is_some())
      .map_or_else(|| CompressedText::literal(literal.to_string()), |entry| CompressedText::table_ref(entry.id))
  }

  /// Replaces inbound table state with an advertised generation.
  ///
  /// # Errors
  ///
  /// Returns [`WireError::InvalidFormat`] when the advertisement contains duplicate entry ids.
  pub fn apply_advertisement(&mut self, generation: u64, entries: &[CompressionTableEntry]) -> Result<(), WireError> {
    if has_duplicate_entry_id(entries) {
      return Err(WireError::InvalidFormat);
    }

    self.entries.clear();
    self.entries.extend(entries.iter().map(|entry| CompressionTableEntryState::from_advertisement(entry, generation)));
    self.latest_pending_generation = None;
    Ok(())
  }

  /// Resolves an inbound table reference id to a literal value.
  #[must_use]
  pub fn resolve(&self, entry_id: u32) -> Option<&str> {
    self.entries.iter().find(|entry| entry.id == entry_id).map(|entry| entry.literal.as_str())
  }
}

fn has_duplicate_entry_id(entries: &[CompressionTableEntry]) -> bool {
  for (index, entry) in entries.iter().enumerate() {
    if entries[index + 1..].iter().any(|candidate| candidate.id() == entry.id()) {
      return true;
    }
  }
  false
}
