use alloc::vec::Vec;

use super::DeadLineTimerKeyAllocator;

#[test]
fn deadline_timer_key_allocator_new() {
  let allocator = DeadLineTimerKeyAllocator::new();
  let first_key = allocator.peek();
  assert!(first_key.is_valid());
  assert_eq!(first_key.into_raw(), 1);
}

#[test]
fn deadline_timer_key_allocator_default() {
  let allocator = DeadLineTimerKeyAllocator::default();
  let first_key = allocator.peek();
  assert_eq!(first_key.into_raw(), 1);
}

#[test]
fn deadline_timer_key_allocator_allocate() {
  let allocator = DeadLineTimerKeyAllocator::new();
  let key1 = allocator.allocate();
  let key2 = allocator.allocate();
  let key3 = allocator.allocate();

  assert!(key1.is_valid());
  assert!(key2.is_valid());
  assert!(key3.is_valid());

  assert_eq!(key1.into_raw(), 1);
  assert_eq!(key2.into_raw(), 2);
  assert_eq!(key3.into_raw(), 3);
}

#[test]
fn deadline_timer_key_allocator_peek() {
  let allocator = DeadLineTimerKeyAllocator::new();
  let initial = allocator.peek();
  assert_eq!(initial.into_raw(), 1);

  let _key1 = allocator.allocate();
  let after_one = allocator.peek();
  assert_eq!(after_one.into_raw(), 2);

  let _key2 = allocator.allocate();
  let after_two = allocator.peek();
  assert_eq!(after_two.into_raw(), 3);
}

#[test]
fn deadline_timer_key_allocator_unique_keys() {
  let allocator = DeadLineTimerKeyAllocator::new();
  let keys: Vec<_> = (0..10).map(|_| allocator.allocate()).collect();

  for (i, key) in keys.iter().enumerate() {
    assert_eq!(key.into_raw(), (i + 1) as u64);
  }
}

#[test]
fn deadline_timer_key_allocator_debug() {
  let allocator = DeadLineTimerKeyAllocator::new();
  let debug_str = format!("{:?}", allocator);
  assert!(debug_str.contains("DeadLineTimerKeyAllocator"));
}
