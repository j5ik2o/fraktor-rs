//! Registry tracking cancellable entries by handle identifier.

use fraktor_utils_rs::core::sync::ArcShared;
use hashbrown::HashMap;

use super::cancellable_entry::CancellableEntry;

/// Stores cancellable entries for active scheduler handles.
#[derive(Default)]
pub struct CancellableRegistry {
  entries: HashMap<u64, ArcShared<CancellableEntry>>,
}

impl CancellableRegistry {
  /// Inserts a new entry for the provided handle.
  pub fn register(&mut self, handle_id: u64, entry: ArcShared<CancellableEntry>) {
    self.entries.insert(handle_id, entry);
  }

  /// Retrieves the entry for the handle, if any.
  #[must_use]
  pub fn get(&self, handle_id: u64) -> Option<ArcShared<CancellableEntry>> {
    self.entries.get(&handle_id).cloned()
  }

  /// Removes the entry for the handle, returning it when present.
  pub fn remove(&mut self, handle_id: u64) -> Option<ArcShared<CancellableEntry>> {
    self.entries.remove(&handle_id)
  }
}
