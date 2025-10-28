use super::deadline_timer_key::DeadlineTimerKey;

/// DeadlineTimer expiration event.
#[derive(Debug)]
pub struct DeadlineTimerExpired<Item> {
  /// The key of the expired item.
  pub key:  DeadlineTimerKey,
  /// The expired item itself.
  pub item: Item,
}
