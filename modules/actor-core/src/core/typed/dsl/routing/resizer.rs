//! Pool router resizer for dynamic routee scaling.

/// Decides when and how to resize a pool router's routee pool.
///
/// Inspired by Pekko's `Resizer` trait. The pool router consults
/// [`Resizer::is_time_for_resize`] on every message; when that returns `true`
/// it then calls [`Resizer::report_message_count`] followed by
/// [`Resizer::resize`] with a shared mailbox snapshot — matching the
/// `ResizablePoolCell.sendMessage` → `tryReportMessageCount` → `resize`
/// sequence in `org.apache.pekko.routing.Resizer`.
pub trait Resizer: Send + Sync {
  /// Returns `true` when the pool should evaluate resizing.
  ///
  /// Invoked by the pool router on every message. `message_counter` starts
  /// at 0 for the first message and increments with each subsequent message.
  fn is_time_for_resize(&self, message_counter: u64) -> bool;

  /// Returns the number of routees to add (positive) or remove (negative).
  ///
  /// `mailbox_sizes[i]` is the observed mailbox length of routee `i` at the
  /// moment the resizer is consulted. The slice length equals the current
  /// number of routees in the pool. Zero means no change.
  fn resize(&self, mailbox_sizes: &[usize]) -> i32;

  /// Records mailbox statistics for a resize tick.
  ///
  /// Called by the pool router immediately before [`Resizer::resize`], and
  /// only when [`Resizer::is_time_for_resize`] returned `true` for the same
  /// message — i.e. once per resize tick, not once per message. This matches
  /// Pekko's `ResizablePoolCell` where `tryReportMessageCount` is invoked
  /// inside the `Resize` handler that `isTimeForResize` schedules.
  ///
  /// The default implementation is a no-op; throughput-aware resizers such
  /// as `OptimalSizeExploringResizer` override it to fold fresh samples
  /// into their performance log.
  ///
  /// `mailbox_sizes[i]` is the observed mailbox length of routee `i` at the
  /// moment of the call. `message_counter` is the cumulative number of
  /// messages dispatched through the router.
  fn report_message_count(&self, _mailbox_sizes: &[usize], _message_counter: u64) {}
}
