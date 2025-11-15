use super::DeadLineTimerKeyAllocator;

#[test]
fn allocate_increments_keys() {
  let allocator = DeadLineTimerKeyAllocator::new();
  let first = allocator.allocate();
  let second = allocator.allocate();
  assert!(first.is_valid());
  assert!(second.is_valid());
  assert_ne!(first.into_raw(), second.into_raw());
}
