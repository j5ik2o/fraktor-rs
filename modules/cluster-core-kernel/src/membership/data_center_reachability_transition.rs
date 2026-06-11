//! Data center reachability transition output.

use super::DataCenter;

/// Latched transition output for data center level reachability changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataCenterReachabilityTransition {
  /// The data center became unreachable (all observed targets unreachable).
  BecameUnreachable {
    /// The data center whose reachability changed.
    data_center: DataCenter,
  },
  /// The data center became reachable again (at least one target reachable).
  BecameReachable {
    /// The data center whose reachability changed.
    data_center: DataCenter,
  },
}
