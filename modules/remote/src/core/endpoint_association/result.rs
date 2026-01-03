//! Result returned after handling a command.

use alloc::vec::Vec;

use super::effect::EndpointAssociationEffect;

/// Result returned after handling a command.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct EndpointAssociationResult {
  /// Side effects produced while handling the command.
  pub effects: Vec<EndpointAssociationEffect>,
}
