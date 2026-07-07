//! Durable store load-all completion protocol message.

/// Marker indicating that startup load has completed.
///
/// This mirrors Pekko's `DurableStore.LoadAllCompleted` message at the port level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DurableStoreLoadAllCompleted;
