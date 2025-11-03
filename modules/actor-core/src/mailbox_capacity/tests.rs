use core::num::NonZeroUsize;

use super::MailboxCapacity;

#[test]
fn bounded_capacity_stores_limit() {
  let limit = NonZeroUsize::new(64).unwrap();
  let capacity = MailboxCapacity::Bounded { capacity: limit };
  match capacity {
    | MailboxCapacity::Bounded { capacity: stored } => assert_eq!(stored, limit),
    | MailboxCapacity::Unbounded => panic!("expected bounded capacity"),
  }
}

#[test]
fn unbounded_capacity_variant() {
  assert!(matches!(MailboxCapacity::Unbounded, MailboxCapacity::Unbounded));
}
