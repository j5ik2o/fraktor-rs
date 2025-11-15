extern crate alloc;

use super::{SyncStack, SyncStackShared};
use crate::{
  collections::stack::backend::{PushOutcome, StackError, StackOverflowPolicy, VecStackBackend},
  sync::{ArcShared, SharedError, sync_mutex_like::SpinSyncMutex},
};

fn make_stack<T>(capacity: usize, policy: StackOverflowPolicy) -> SyncStackShared<T, VecStackBackend<T>> {
  let backend = VecStackBackend::with_capacity(capacity, policy);
  let sync_stack = SyncStack::new(backend);
  let shared = ArcShared::new(SpinSyncMutex::new(sync_stack));
  SyncStackShared::new(shared)
}

#[test]
fn push_pop_maintains_lifo() {
  let stack = make_stack(2, StackOverflowPolicy::Block);

  assert_eq!(stack.push(1).unwrap(), PushOutcome::Pushed);
  assert_eq!(stack.push(2).unwrap(), PushOutcome::Pushed);
  assert_eq!(stack.pop().unwrap(), 2);
  assert_eq!(stack.pop().unwrap(), 1);
  assert!(matches!(stack.pop(), Err(StackError::Empty)));
}

#[test]
fn block_policy_reports_full() {
  let stack = make_stack(1, StackOverflowPolicy::Block);

  assert_eq!(stack.push(10).unwrap(), PushOutcome::Pushed);
  let err = stack.push(20).unwrap_err();
  assert_eq!(err, StackError::Full);
}

#[test]
fn grow_policy_increases_capacity() {
  let stack = make_stack(1, StackOverflowPolicy::Grow);

  assert_eq!(stack.push(1).unwrap(), PushOutcome::Pushed);
  let outcome = stack.push(2).unwrap();
  assert!(matches!(outcome, PushOutcome::GrewTo { capacity: 2 }));
  assert_eq!(stack.capacity(), 2);

  stack.close().unwrap();
  assert!(matches!(stack.push(3), Err(StackError::Closed)));
  assert_eq!(stack.pop().unwrap(), 2);
  assert_eq!(stack.pop().unwrap(), 1);
  assert!(matches!(stack.pop(), Err(StackError::Closed)));
}

#[test]
fn peek_returns_top_element() {
  let stack = make_stack(3, StackOverflowPolicy::Block);

  assert_eq!(stack.peek().unwrap(), None);
  stack.push(5).unwrap();
  stack.push(7).unwrap();
  assert_eq!(stack.peek().unwrap(), Some(7));
  assert_eq!(stack.pop().unwrap(), 7);
  assert_eq!(stack.peek().unwrap(), Some(5));
}

#[test]
fn shared_error_mapping_matches_spec() {
  assert_eq!(StackError::from(SharedError::Poisoned), StackError::Disconnected);
  assert_eq!(StackError::from(SharedError::BorrowConflict), StackError::WouldBlock);
  assert_eq!(StackError::from(SharedError::InterruptContext), StackError::WouldBlock);
}
