//! Result classification for applying pub-sub registry deltas.

use fraktor_remote_core_rs::address::UniqueAddress;

use crate::pub_sub::TopicRegistryVersion;

/// Observable outcome of applying one registry delta entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopicRegistryApplyOutcome {
  /// Delta entry was applied.
  Applied {
    /// Owner bucket identity.
    owner:   UniqueAddress,
    /// Applied entry version.
    version: TopicRegistryVersion,
  },
  /// Delta entry was ignored because the owner is not active.
  IgnoredInactiveOwner {
    /// Owner bucket identity.
    owner: UniqueAddress,
  },
  /// Delta entry was ignored because the local entry is newer or equal.
  IgnoredStale {
    /// Owner bucket identity.
    owner:   UniqueAddress,
    /// Ignored entry version.
    version: TopicRegistryVersion,
  },
}
