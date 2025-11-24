//! Identity lookup abstraction for cluster modes.

use crate::core::{activated_kind::ActivatedKind, identity_setup_error::IdentitySetupError};

/// Provides identity resolution setup for member and client modes.
pub trait IdentityLookup: Send + Sync {
  /// Prepares identity lookup for member mode with the provided kinds.
  fn setup_member(&self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError>;

  /// Prepares identity lookup for client mode with the provided kinds.
  fn setup_client(&self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError>;
}
