//! Startup/shutdown mode of the cluster runtime.

/// Indicates how the cluster runtime is booted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupMode {
  /// Member node mode.
  Member,
  /// Client node mode.
  Client,
}
