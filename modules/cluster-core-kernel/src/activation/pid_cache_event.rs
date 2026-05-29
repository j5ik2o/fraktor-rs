//! Events emitted from cache operations.

use alloc::string::String;

use crate::grain::GrainKey;

/// Events emitted from cache operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PidCacheEvent {
  /// Entry was dropped due to TTL or owner change.
  Dropped {
    /// Key that was removed.
    key:    GrainKey,
    /// Context of removal.
    reason: String,
  },
}
