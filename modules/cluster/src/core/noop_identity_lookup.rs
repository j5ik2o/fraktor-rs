//! No-op implementation of the IdentityLookup trait.

use crate::core::{
  activated_kind::ActivatedKind, identity_lookup::IdentityLookup, identity_setup_error::IdentitySetupError,
};

/// A no-op identity lookup that does nothing.
///
/// This implementation is useful for testing, single-node clusters,
/// or scenarios where identity lookup is not required.
///
/// All methods that modify state are no-ops. Methods with default implementations
/// in the trait are inherited.
#[derive(Clone, Debug, Default)]
pub struct NoopIdentityLookup;

impl NoopIdentityLookup {
  /// Creates a new no-op identity lookup.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl IdentityLookup for NoopIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }
  // get, remove_pid, update_topology, on_member_left, passivate_idle, drain_events,
  // drain_cache_events はトレイトのデフォルト実装をそのまま継承
}
