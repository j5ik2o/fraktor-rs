//! Error type for ECS polling operations.

use std::string::String;

/// Error type for ECS polling operations.
#[derive(Debug)]
pub enum EcsPollerError {
  /// API call failed.
  ApiCall(String),
}
