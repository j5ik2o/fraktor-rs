use super::StreamDslError;

/// Validates that the provided argument is greater than zero.
///
/// # Errors
///
/// Returns [`StreamDslError::InvalidArgument`] when `value == 0`.
pub const fn validate_positive_argument(name: &'static str, value: usize) -> Result<usize, StreamDslError> {
  if value == 0 {
    return Err(StreamDslError::InvalidArgument { name, value, reason: "must be greater than zero" });
  }
  Ok(value)
}
