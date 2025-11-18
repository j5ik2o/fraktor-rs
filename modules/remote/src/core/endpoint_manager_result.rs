//! Result returned after handling a command.

use alloc::vec::Vec;

use crate::core::endpoint_manager_effect::EndpointManagerEffect;

/// Result returned after handling a command.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct EndpointManagerResult {
  /// Side effects produced while handling the command.
  pub effects: Vec<EndpointManagerEffect>,
}
