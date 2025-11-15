use core::{
  future::Future,
  pin::Pin,
  ptr,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use super::AsyncStackShared;
use crate::{
  collections::stack::{
    AsyncStack, StackOverflowPolicy,
    backend::{PushOutcome, StackError, SyncStackAsyncAdapter, VecStackBackend},
  },
  sync::{ArcShared, SharedError, async_mutex_like::SpinAsyncMutex, interrupt::InterruptContextPolicy},
};

fn raw_waker() -> RawWaker {
  fn clone(_: *const ()) -> RawWaker {
    raw_waker()
  }
  fn wake(_: *const ()) {}
  fn wake_by_ref(_: *const ()) {}
  fn drop(_: *const ()) {}
  static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
  RawWaker::new(ptr::null(), &VTABLE)
}

fn noop_waker() -> Waker {
  unsafe { Waker::from_raw(raw_waker()) }
}

fn block_on<F: Future>(mut future: F) -> F::Output {
  let waker = noop_waker();
  let mut future = unsafe { Pin::new_unchecked(&mut future) };
  let mut context = Context::from_waker(&waker);

  loop {
    match future.as_mut().poll(&mut context) {
      | Poll::Ready(output) => return output,
      | Poll::Pending => continue,
    }
  }
}

type SharedStack = ArcShared<SpinAsyncMutex<AsyncStack<i32, SyncStackAsyncAdapter<i32, VecStackBackend<i32>>>>>;

fn make_shared_stack(capacity: usize, policy: StackOverflowPolicy) -> SharedStack {
  let backend = VecStackBackend::with_capacity(capacity, policy);
  let async_stack = AsyncStack::new(SyncStackAsyncAdapter::new(backend));
  ArcShared::new(SpinAsyncMutex::new(async_stack))
}

struct DenyPolicy;

impl InterruptContextPolicy for DenyPolicy {
  fn check_blocking_allowed() -> Result<(), SharedError> {
    Err(SharedError::InterruptContext)
  }
}

type DenyMutex<T> = SpinAsyncMutex<T, DenyPolicy>;
type DenySharedStack = ArcShared<DenyMutex<AsyncStack<i32, SyncStackAsyncAdapter<i32, VecStackBackend<i32>>>>>;

fn make_interrupt_shared_stack(capacity: usize) -> DenySharedStack {
  let backend = VecStackBackend::with_capacity(capacity, StackOverflowPolicy::Block);
  let async_stack = AsyncStack::new(SyncStackAsyncAdapter::new(backend));
  ArcShared::new(DenyMutex::new(async_stack))
}

#[test]
fn push_and_pop_operates_async_stack() {
  let shared = make_shared_stack(4, StackOverflowPolicy::Block);
  let stack: AsyncStackShared<i32, _, _> = AsyncStackShared::new(shared);

  assert!(matches!(block_on(stack.push(10)), Ok(PushOutcome::Pushed)));
  assert_eq!(block_on(stack.len()), Ok(1));
  assert_eq!(block_on(stack.pop()), Ok(10));

  let mut pending_pop = stack.pop();
  let mut pending_pop = unsafe { Pin::new_unchecked(&mut pending_pop) };

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert!(matches!(pending_pop.as_mut().poll(&mut context), Poll::Pending));
}

#[test]
fn peek_reflects_top_element() {
  let shared = make_shared_stack(4, StackOverflowPolicy::Block);
  let stack: AsyncStackShared<i32, _, _> = AsyncStackShared::new(shared);

  assert!(matches!(block_on(stack.push(1)), Ok(PushOutcome::Pushed)));
  assert!(matches!(block_on(stack.push(2)), Ok(PushOutcome::Pushed)));
  assert_eq!(block_on(stack.peek()), Ok(Some(2)));
  assert_eq!(block_on(stack.len()), Ok(2));
}

#[test]
fn close_prevents_additional_pushes() {
  let shared = make_shared_stack(2, StackOverflowPolicy::Block);
  let stack: AsyncStackShared<i32, _, _> = AsyncStackShared::new(shared);

  assert!(matches!(block_on(stack.push(5)), Ok(PushOutcome::Pushed)));
  assert!(block_on(stack.close()).is_ok());
  assert_eq!(block_on(stack.push(6)), Err(StackError::Closed));
  assert_eq!(block_on(stack.pop()), Ok(5));
  assert_eq!(block_on(stack.pop()), Err(StackError::Closed));
}

#[test]
fn push_blocks_until_space_available() {
  let shared = make_shared_stack(1, StackOverflowPolicy::Block);
  let stack: AsyncStackShared<i32, _, _> = AsyncStackShared::new(shared);

  assert!(matches!(block_on(stack.push(1)), Ok(PushOutcome::Pushed)));

  let mut push_future = stack.push(2);
  let mut push_future = unsafe { Pin::new_unchecked(&mut push_future) };

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert!(matches!(push_future.as_mut().poll(&mut context), Poll::Pending));

  assert_eq!(block_on(stack.pop()), Ok(1));

  assert!(matches!(push_future.as_mut().poll(&mut context), Poll::Ready(Ok(PushOutcome::Pushed))));
}

#[test]
fn pop_blocks_until_item_available() {
  let shared = make_shared_stack(1, StackOverflowPolicy::Block);
  let stack: AsyncStackShared<i32, _, _> = AsyncStackShared::new(shared);

  let mut pop_future = stack.pop();
  let mut pop_future = unsafe { Pin::new_unchecked(&mut pop_future) };

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert!(matches!(pop_future.as_mut().poll(&mut context), Poll::Pending));

  assert!(matches!(block_on(stack.push(3)), Ok(PushOutcome::Pushed)));

  assert_eq!(pop_future.as_mut().poll(&mut context), Poll::Ready(Ok(3)));
}

#[test]
fn close_wakes_waiting_pop() {
  let shared = make_shared_stack(1, StackOverflowPolicy::Block);
  let stack: AsyncStackShared<i32, _, _> = AsyncStackShared::new(shared);

  let mut pop_future = stack.pop();
  let mut pop_future = unsafe { Pin::new_unchecked(&mut pop_future) };

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert!(matches!(pop_future.as_mut().poll(&mut context), Poll::Pending));

  assert!(block_on(stack.close()).is_ok());

  assert_eq!(pop_future.as_mut().poll(&mut context), Poll::Ready(Err(StackError::Closed)));
}

#[test]
fn interrupt_context_returns_would_block_errors() {
  let shared = make_interrupt_shared_stack(2);
  let stack: AsyncStackShared<i32, _, _> = AsyncStackShared::new(shared);

  assert_eq!(block_on(stack.push(1)), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.pop()), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.peek()), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.close()), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.len()), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.capacity()), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.is_empty()), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.is_full()), Err(StackError::WouldBlock));
}
