//! Pool router resizer for dynamic routee scaling.

/// Decides when and how to resize a pool router's routee pool.
///
/// Inspired by Pekko's `Resizer` trait, this provides two decisions:
/// 1. Whether it is time to check for resizing (evaluated on every message).
/// 2. How many routees to add or remove.
pub trait Resizer: Send + Sync {
  /// Returns `true` when the pool should evaluate resizing.
  ///
  /// `message_counter` starts at 0 for the first message and increments
  /// with each subsequent message.
  fn is_time_for_resize(&self, message_counter: u64) -> bool;

  /// Returns the number of routees to add (positive) or remove (negative).
  ///
  /// Zero means no change.
  fn resize(&self, current_routee_count: usize) -> i32;
}
