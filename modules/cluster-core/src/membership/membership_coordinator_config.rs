//! Configuration for membership coordination.

use core::time::Duration;

/// Configuration for membership coordination and gossip.
#[derive(Clone, Debug)]
pub struct MembershipCoordinatorConfig {
  /// Phi threshold for suspect detection.
  pub phi_threshold:          f64,
  /// Timeout for suspect to dead transition.
  pub suspect_timeout:        Duration,
  /// Timeout for dead to removed transition (reserved).
  pub dead_timeout:           Duration,
  /// Quarantine time-to-live.
  pub quarantine_ttl:         Duration,
  /// Enables gossip dissemination.
  pub gossip_enabled:         bool,
  /// Interval between gossip dissemination attempts.
  pub gossip_interval:        Duration,
  /// Interval for topology update emission.
  pub topology_emit_interval: Duration,
}
