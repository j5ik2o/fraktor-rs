use core::task::{Context, Poll};

use super::{
  dead_line_timer_expired::DeadLineTimerExpired, dead_line_timer_key::DeadLineTimerKey, timer_dead_line::TimerDeadLine,
};

/// Trait abstracting DeadlineTimer behavior.
pub trait DeadLineTimer {
  /// The type of elements held by the timer.
  type Item;
  /// The error type that may occur during operations.
  type Error;

  /// Inserts a new element with a deadline.
  ///
  /// # Errors
  ///
  /// Returns `Self::Error` when the timer cannot store the item, for example due to exhausted
  /// capacity or backend failures.
  fn insert(&mut self, item: Self::Item, deadline: TimerDeadLine) -> Result<DeadLineTimerKey, Self::Error>;

  /// Updates the deadline for an element with the specified key.
  ///
  /// # Errors
  ///
  /// Returns `Self::Error` when the element is missing or the backend rejects the updated deadline.
  fn reset(&mut self, key: DeadLineTimerKey, deadline: TimerDeadLine) -> Result<(), Self::Error>;

  /// Cancels an element with the specified key and returns it.
  ///
  /// # Errors
  ///
  /// Returns `Self::Error` when cancellation fails because the key is invalid or backend
  /// operations fail.
  fn cancel(&mut self, key: DeadLineTimerKey) -> Result<Option<Self::Item>, Self::Error>;

  /// Polls for the element with the closest deadline.
  fn poll_expired(&mut self, cx: &mut Context<'_>) -> Poll<Result<DeadLineTimerExpired<Self::Item>, Self::Error>>;
}
