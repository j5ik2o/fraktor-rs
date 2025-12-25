//! Path identity configuration for actor system addressing.

use alloc::string::{String, ToString};
use core::time::Duration;

use crate::core::actor::actor_path::GuardianKind as PathGuardianKind;

const DEFAULT_SYSTEM_NAME: &str = "fraktor";
pub(crate) const DEFAULT_QUARANTINE_DURATION: Duration = Duration::from_secs(5 * 24 * 3600);

/// Holds the addressing identity of the actor system.
#[derive(Clone)]
pub(crate) struct PathIdentity {
  pub(crate) system_name:         String,
  pub(crate) canonical_host:      Option<String>,
  pub(crate) canonical_port:      Option<u16>,
  pub(crate) quarantine_duration: Duration,
  pub(crate) guardian_kind:       PathGuardianKind,
}

impl Default for PathIdentity {
  fn default() -> Self {
    Self {
      system_name:         DEFAULT_SYSTEM_NAME.to_string(),
      canonical_host:      None,
      canonical_port:      None,
      quarantine_duration: DEFAULT_QUARANTINE_DURATION,
      guardian_kind:       PathGuardianKind::User,
    }
  }
}
