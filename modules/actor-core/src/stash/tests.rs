use cellactor_utils_core_rs::collections::queue::{DequeBackend, OverflowPolicy};

use super::{DequeHandle, StashDequeHandleGeneric};

fn assert_trait_behaviour(handle: &dyn DequeHandle<i32>) {
  handle.push_back(1).expect("push back");
  handle.push_front(0).expect("push front");
  assert_eq!(handle.pop_front().expect("pop front"), 0);
  assert_eq!(handle.pop_back().expect("pop back"), 1);
}

#[test]
fn stash_deque_handle_supports_trait_object_usage() {
  let backend = DequeBackend::with_capacity(4, OverflowPolicy::Block);
  let handle = StashDequeHandleGeneric::new(backend);
  assert_trait_behaviour(&handle);
}

#[test]
fn stash_deque_handle_allows_multiple_push_operations() {
  let backend = DequeBackend::with_capacity(8, OverflowPolicy::Grow);
  let handle = StashDequeHandleGeneric::new(backend);

  handle.push_back(10).expect("push back");
  handle.push_back(11).expect("push back");
  handle.push_front(9).expect("push front");

  assert_eq!(handle.pop_front().expect("pop front"), 9);
  assert_eq!(handle.pop_front().expect("pop front"), 10);
  assert_eq!(handle.pop_back().expect("pop back"), 11);
}
