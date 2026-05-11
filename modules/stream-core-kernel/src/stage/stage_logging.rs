#[cfg(test)]
#[path = "stage_logging_test.rs"]
mod tests;

/// Stage-level logging facade.
///
/// Mirrors Apache Pekko's `pekko.stream.stage.StageLogging`. A stage
/// implementation exposes its logical name through [`log_stage_name`] and
/// forwards messages to the materializer's logger via the five severity
/// methods. The trait is intentionally `&self` only so logging can remain
/// a CQS-query side operation even when the stage is held immutably.
///
/// Implementations are expected to route messages through the
/// `MaterializerLoggingProvider` registered on the materializer; the
/// trait itself does not prescribe a backend.
pub trait StageLogging {
  /// Returns the logical stage name used as the log source.
  fn log_stage_name(&self) -> &str;

  /// Emits a `TRACE` level message.
  fn log_trace(&self, msg: &str);

  /// Emits a `DEBUG` level message.
  fn log_debug(&self, msg: &str);

  /// Emits an `INFO` level message.
  fn log_info(&self, msg: &str);

  /// Emits a `WARN` level message.
  fn log_warn(&self, msg: &str);

  /// Emits an `ERROR` level message.
  fn log_error(&self, msg: &str);
}
