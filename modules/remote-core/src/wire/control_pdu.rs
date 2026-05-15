//! Control PDU: heartbeat / quarantine / shutdown / flush signalling.

use alloc::{string::String, vec::Vec};

use super::{CompressionTableEntry, CompressionTableKind, FlushScope};

/// Wire-level control PDU carrying non-envelope signalling between nodes.
///
/// Each variant shares the same frame `kind = 0x04` and is differentiated by an
/// inner `subkind` byte at the start of the body (`0x00 = Heartbeat`,
/// `0x01 = Quarantine`, `0x02 = Shutdown`, `0x03 = HeartbeatResponse`,
/// `0x04 = FlushRequest`, `0x05 = FlushAck`,
/// `0x06 = CompressionAdvertisement`, `0x07 = CompressionAck`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlPdu {
  /// Periodic liveness signal from the sending node.
  Heartbeat {
    /// Authority string (typically the sender's canonical address).
    authority: String,
  },
  /// Liveness response carrying the sender's actor-system incarnation UID.
  HeartbeatResponse {
    /// Authority string (typically the sender's canonical address).
    authority: String,
    /// Actor-system incarnation UID of the responding node.
    uid:       u64,
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
  /// Request that a peer flush pending outbound work for a lane.
  FlushRequest {
    /// Authority string (typically the sender's canonical address).
    authority:     String,
    /// Monotonic flush identifier chosen by the requester.
    flush_id:      u64,
    /// Reason and ordering scope for this flush.
    scope:         FlushScope,
    /// TCP lane id whose prior frames must be observed.
    lane_id:       u32,
    /// Number of acknowledgements expected by the requester.
    expected_acks: u32,
  },
  /// Acknowledgement for a completed lane flush.
  FlushAck {
    /// Authority string (typically the sender's canonical address).
    authority:     String,
    /// Flush identifier echoed from the request.
    flush_id:      u64,
    /// TCP lane id acknowledged by this response.
    lane_id:       u32,
    /// Number of acknowledgements expected by the requester.
    expected_acks: u32,
  },
  /// Advertisement of compression table entries.
  CompressionAdvertisement {
    /// Authority string (typically the sender's canonical address).
    authority:  String,
    /// Compression table kind.
    table_kind: CompressionTableKind,
    /// Advertisement generation.
    generation: u64,
    /// Advertised entries.
    entries:    Vec<CompressionTableEntry>,
  },
  /// Acknowledgement for a compression table advertisement.
  CompressionAck {
    /// Authority string (typically the sender's canonical address).
    authority:  String,
    /// Compression table kind.
    table_kind: CompressionTableKind,
    /// Acknowledged advertisement generation.
    generation: u64,
  },
}
