//! State of an outbound authority channel.

use alloc::string::String;

#[cfg(test)]
mod tests;

/// Connection state for a single authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutboundState {
  /// Authority is disconnected; messages are buffered.
  Disconnected,
  /// Authority is connected; messages are dispatched immediately.
  Connected,
  /// Authority is quarantined and rejects all traffic.
  Quarantine {
    /// Human-readable reason.
    reason:   String,
    /// Optional deadline (monotonic seconds) when quarantine is lifted.
    deadline: Option<u64>,
  },
}

impl OutboundState {
  /// Returns true when the authority currently rejects traffic.
  #[must_use]
  pub const fn is_blocking(&self) -> bool {
    matches!(self, Self::Quarantine { .. })
  }
}
