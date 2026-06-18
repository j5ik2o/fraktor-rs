use core::{num::NonZeroUsize, time::Duration};

use super::MailboxPolicy;
use crate::dispatch::mailbox::{MailboxCapacity, MailboxOverflowStrategy};

#[test]
fn bounded_policy_reports_settings() {
  let capacity = NonZeroUsize::new(32).unwrap();
  let policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None);
  assert!(matches!(policy.capacity(), MailboxCapacity::Bounded { capacity: stored } if stored == capacity));
  assert_eq!(policy.overflow(), MailboxOverflowStrategy::DropNewest);
  assert_eq!(policy.throughput_limit(), None);
}

#[test]
fn unbounded_policy_defaults_to_drop_oldest() {
  let policy = MailboxPolicy::unbounded(None);
  assert_eq!(policy.capacity(), MailboxCapacity::Unbounded);
  assert_eq!(policy.overflow(), MailboxOverflowStrategy::DropOldest);
}

#[test]
fn with_overrides_return_new_values() {
  let capacity = NonZeroUsize::new(16).unwrap();
  let policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropOldest, None)
    .with_overflow(MailboxOverflowStrategy::Grow)
    .with_throughput_limit(NonZeroUsize::new(8))
    .with_push_timeout(Some(Duration::from_millis(25)))
    .with_capacity(MailboxCapacity::Unbounded);

  assert_eq!(policy.capacity(), MailboxCapacity::Unbounded);
  assert_eq!(policy.overflow(), MailboxOverflowStrategy::Grow);
  assert_eq!(policy.throughput_limit(), NonZeroUsize::new(8));
  assert_eq!(policy.push_timeout(), Some(Duration::from_millis(25)));
}

#[test]
fn push_timeout_defaults_to_none_and_can_be_overridden() {
  let capacity = NonZeroUsize::new(8).unwrap();
  let policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None);

  assert_eq!(policy.push_timeout(), None);
  assert_eq!(policy.with_push_timeout(Some(Duration::ZERO)).push_timeout(), Some(Duration::ZERO));
}
