//! Control PDU: heartbeat / quarantine / shutdown signalling.

use alloc::string::String;

/// Wire-level control PDU carrying non-envelope signalling between nodes.
///
/// Each variant shares the same frame `kind = 0x04` and is differentiated by an
/// inner `subkind` byte at the start of the body (`0x00 = Heartbeat`,
/// `0x01 = Quarantine`, `0x02 = Shutdown`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlPdu {
  /// Periodic liveness signal from the sending node.
  Heartbeat {
    /// Authority string (typically the sender's canonical address).
    authority: String,
  },
  /// Notification that the sending node has quarantined a peer.
  Quarantine {
    /// Authority string of the quarantined peer.
    authority: String,
    /// Optional human-readable reason.
    reason:    Option<String>,
  },
  /// Notification that the sending node is shutting down.
  Shutdown {
    /// Authority string (typically the sender's canonical address).
    authority: String,
  },
}
