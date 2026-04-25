use crate::core::StreamError;

/// Result of applying stream-stage failure handling.
pub(in crate::core) enum FailureDisposition {
  /// Continue processing after the failure has been handled.
  Continue,
  /// Complete the affected stream path.
  Complete,
  /// Fail the stream with the preserved payload.
  Fail(StreamError),
}
