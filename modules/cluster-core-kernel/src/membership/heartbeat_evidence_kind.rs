//! Heartbeat evidence category.

/// Liveness evidence produced by the dedicated heartbeat protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatEvidenceKind {
  /// A matching response arrived with measured latency.
  Reachable {
    /// Response latency in ticks or milliseconds chosen by the caller clock.
    latency_ms: u64,
  },
  /// The first expected heartbeat did not arrive before the first deadline.
  FirstMissed,
  /// A later heartbeat response did not arrive before its deadline.
  Missed,
}
