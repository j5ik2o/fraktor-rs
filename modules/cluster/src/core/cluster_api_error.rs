//! Errors returned when acquiring the cluster API.

/// Errors raised while retrieving the cluster API from the actor system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterApiError {
  /// The cluster extension has not been installed.
  ExtensionNotInstalled,
}
