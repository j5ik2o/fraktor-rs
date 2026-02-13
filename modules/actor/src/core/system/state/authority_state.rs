//! Remote authority state definitions.

/// State of a remote authority.
#[derive(Clone, Debug, PartialEq)]
pub enum AuthorityState {
  /// Authority has not been resolved yet; messages are deferred.
  Unresolved,
  /// Authority is connected and ready to accept messages.
  Connected,
  /// Authority is quarantined; new sends are rejected.
  Quarantine {
    /// Absolute deadline (monotonic time) when quarantine should be lifted.
    deadline: Option<u64>,
  },
}
