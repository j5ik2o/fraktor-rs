//! Durable store load-all request protocol message.

/// Request to load all durable distributed-data entries.
///
/// This mirrors Pekko's `DurableStore.LoadAll` message at the port level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DurableStoreLoadAll;
