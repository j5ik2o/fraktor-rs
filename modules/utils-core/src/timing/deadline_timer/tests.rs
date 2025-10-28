#![allow(clippy::disallowed_types)]
extern crate std;

use std::{collections::HashSet, time::Duration};

use super::*;

#[test]
fn allocate_provides_unique_keys() {
  let allocator = DeadlineTimerKeyAllocator::new();
  let mut keys = HashSet::new();

  for _ in 0..1024 {
    let key = allocator.allocate();
    assert!(key.is_valid());
    assert!(keys.insert(key.into_raw()));
  }
}

#[test]
fn deadline_roundtrip() {
  let duration = Duration::from_millis(150);
  let deadline = TimerDeadline::from(duration);
  assert_eq!(deadline.as_duration(), duration);
  let back: Duration = deadline.into();
  assert_eq!(back, duration);
}
