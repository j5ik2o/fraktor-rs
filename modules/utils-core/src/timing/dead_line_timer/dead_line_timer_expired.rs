use super::dead_line_timer_key::DeadLineTimerKey;

/// DeadlineTimer expiration event.
#[derive(Debug)]
pub struct DeadLineTimerExpired<Item> {
  /// The key of the expired item.
  pub key: DeadLineTimerKey,
  /// The expired item itself.
  pub item: Item,
}
