//! Pool router resizer for dynamic routee scaling.

/// Decides when and how to resize a pool router's routee pool.
///
/// Inspired by Pekko's `Resizer` trait, this provides three decisions:
/// 1. Whether it is time to check for resizing (evaluated on every message).
/// 2. How many routees to add or remove.
/// 3. Optional per-message observation of routee mailbox sizes.
pub trait Resizer: Send + Sync {
  /// Returns `true` when the pool should evaluate resizing.
  ///
  /// `message_counter` starts at 0 for the first message and increments
  /// with each subsequent message.
  fn is_time_for_resize(&self, message_counter: u64) -> bool;

  /// Returns the number of routees to add (positive) or remove (negative).
  ///
  /// `mailbox_sizes[i]` is the observed mailbox length of routee `i` at the
  /// moment the resizer is consulted. The slice length equals the current
  /// number of routees in the pool. Zero means no change.
  fn resize(&self, mailbox_sizes: &[usize]) -> i32;

  /// Records per-message mailbox statistics.
  ///
  /// Called on every message when a resizer is attached, regardless of
  /// whether [`Resizer::is_time_for_resize`] returns `true`. The default
  /// implementation is a no-op; throughput-aware resizers such as
  /// `OptimalSizeExploringResizer` override this to collect metrics.
  ///
  /// `mailbox_sizes[i]` is the observed mailbox length of routee `i` at the
  /// moment of the call. `message_counter` is the cumulative number of
  /// messages dispatched through the router.
  fn report_message_count(&self, _mailbox_sizes: &[usize], _message_counter: u64) {}
}
