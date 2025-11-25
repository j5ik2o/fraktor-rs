//! No-op implementation of the IdentityLookup trait.

use crate::core::{
  activated_kind::ActivatedKind, identity_lookup::IdentityLookup, identity_setup_error::IdentitySetupError,
};

/// A no-op identity lookup that does nothing.
///
/// This implementation is useful for testing, single-node clusters,
/// or scenarios where identity lookup is not required.
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
  fn setup_member(&self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }
}
