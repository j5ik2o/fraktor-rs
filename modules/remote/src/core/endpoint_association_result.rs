//! Result returned after handling a command.

use alloc::vec::Vec;

use crate::core::endpoint_association_effect::EndpointAssociationEffect;

/// Result returned after handling a command.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct EndpointAssociationResult {
  /// Side effects produced while handling the command.
  pub effects: Vec<EndpointAssociationEffect>,
}
