use crate::{DemandTracker, DynValue, SinkDecision, StreamError, stream_ref::StreamRefSettings};

/// Sink-stage callback contract used by adaptor implementations.
///
/// Implementations must manage demand through the provided [`DemandTracker`]:
///
/// - [`on_start`](SinkLogic::on_start) must enqueue the initial demand via [`DemandTracker`] before
///   the first element arrives.
/// - [`on_push`](SinkLogic::on_push) must restore demand via [`DemandTracker`] as needed before
///   returning [`SinkDecision::Continue`] so the stream keeps flowing.
pub trait SinkLogic: Send {
  /// Returns whether the sink can accept another input element.
  fn can_accept_input(&self) -> bool {
    true
  }

  /// Initializes sink state before the first element arrives.
  ///
  /// The implementation must enqueue the initial demand via [`DemandTracker`]
  /// so the source begins producing elements.
  ///
  /// # Errors
  ///
  /// Returns a [`StreamError`] when initialization fails.
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError>;
  /// Consumes one input element.
  ///
  /// After processing, the implementation should top-up demand via
  /// [`DemandTracker`] before returning [`SinkDecision::Continue`].
  ///
  /// # Errors
  ///
  /// Returns a [`StreamError`] when the input cannot be processed.
  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError>;
  /// Completes the sink successfully.
  ///
  /// # Errors
  ///
  /// Returns a [`StreamError`] when completion cleanup fails.
  fn on_complete(&mut self) -> Result<(), StreamError>;
  /// Handles terminal stream failure.
  fn on_error(&mut self, error: StreamError);
  /// Handles timer ticks for sinks with scheduled work.
  ///
  /// # Errors
  ///
  /// Returns a [`StreamError`] when timer-driven work fails.
  fn on_tick(&mut self, _demand: &mut DemandTracker) -> Result<bool, StreamError> {
    Ok(false)
  }

  /// Handles upstream completion before the sink has finished its own work.
  ///
  /// # Errors
  ///
  /// Returns a [`StreamError`] when finish handling fails.
  fn on_upstream_finish(&mut self) -> Result<bool, StreamError> {
    Ok(false)
  }

  /// Returns whether the sink still has buffered work to flush.
  fn has_pending_work(&self) -> bool {
    false
  }

  /// Resets internal state after a restart decision.
  ///
  /// # Errors
  ///
  /// Returns a [`StreamError`] when restart cleanup fails.
  fn on_restart(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  /// Attaches stream reference settings resolved by the materializer.
  fn attach_stream_ref_settings(&mut self, _settings: StreamRefSettings) {}
}
