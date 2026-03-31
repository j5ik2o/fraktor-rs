//! Error returned by typed ask futures.

use crate::core::{kernel::actor::messaging::AskError, typed::dsl::StatusReplyError};

/// Reports failures during typed ask resolution.
#[derive(Clone, Debug)]
pub enum TypedAskError {
  /// The reply payload could not be converted to the requested type `R`.
  TypeMismatch,
  /// The reply payload is still shared elsewhere and cannot be moved out.
  SharedReferences,
  /// The underlying ask operation failed.
  AskFailed(AskError),
  /// The responder returned a `StatusReply::Error` with a domain-level failure reason.
  StatusError(StatusReplyError),
}

impl core::fmt::Display for TypedAskError {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      | TypedAskError::TypeMismatch => f.write_str("typed ask received unexpected reply type"),
      | TypedAskError::SharedReferences => f.write_str("typed ask reply still has outstanding references"),
      | TypedAskError::AskFailed(ask_error) => write!(f, "typed ask failed: {ask_error}"),
      | TypedAskError::StatusError(status_err) => write!(f, "typed ask status error: {status_err}"),
    }
  }
}

impl From<AskError> for TypedAskError {
  fn from(error: AskError) -> Self {
    TypedAskError::AskFailed(error)
  }
}

impl From<StatusReplyError> for TypedAskError {
  fn from(error: StatusReplyError) -> Self {
    TypedAskError::StatusError(error)
  }
}
