use core::{
  future::Future,
  pin::Pin,
  ptr,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use super::AsyncStack;
use crate::{
  collections::stack::{
    StackOverflowPolicy, VecStackStorage,
    backend::{PushOutcome, StackError, SyncAdapterStackBackend, VecStackBackend},
  },
  sync::{ArcShared, async_mutex_like::SpinAsyncMutex, interrupt::InterruptContextPolicy, shared_error::SharedError},
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

fn make_shared_stack(
  capacity: usize,
  policy: StackOverflowPolicy,
) -> ArcShared<SpinAsyncMutex<SyncAdapterStackBackend<i32, VecStackBackend<i32>>>> {
  let storage = VecStackStorage::with_capacity(capacity);
  let backend = VecStackBackend::new_with_storage(storage, policy);
  ArcShared::new(SpinAsyncMutex::new(SyncAdapterStackBackend::new(backend)))
}

struct DenyPolicy;

impl InterruptContextPolicy for DenyPolicy {
  fn check_blocking_allowed() -> Result<(), SharedError> {
    Err(SharedError::InterruptContext)
  }
}

type DenyMutex<T> = SpinAsyncMutex<T, DenyPolicy>;

fn make_interrupt_shared_stack(
  capacity: usize,
) -> ArcShared<DenyMutex<SyncAdapterStackBackend<i32, VecStackBackend<i32>>>> {
  let storage = VecStackStorage::with_capacity(capacity);
  let backend = VecStackBackend::new_with_storage(storage, StackOverflowPolicy::Block);
  ArcShared::new(DenyMutex::new(SyncAdapterStackBackend::new(backend)))
}

#[test]
fn push_and_pop_operates_async_stack() {
  let shared = make_shared_stack(4, StackOverflowPolicy::Block);
  let stack: AsyncStack<i32, _, _> = AsyncStack::new(shared);

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
  let stack: AsyncStack<i32, _, _> = AsyncStack::new(shared);

  assert!(matches!(block_on(stack.push(1)), Ok(PushOutcome::Pushed)));
  assert!(matches!(block_on(stack.push(2)), Ok(PushOutcome::Pushed)));
  assert_eq!(block_on(stack.peek()), Ok(Some(2)));
  assert_eq!(block_on(stack.len()), Ok(2));
}

#[test]
fn close_prevents_additional_pushes() {
  let shared = make_shared_stack(2, StackOverflowPolicy::Block);
  let stack: AsyncStack<i32, _, _> = AsyncStack::new(shared);

  assert!(matches!(block_on(stack.push(5)), Ok(PushOutcome::Pushed)));
  assert!(block_on(stack.close()).is_ok());
  assert_eq!(block_on(stack.push(6)), Err(StackError::Closed));
  assert_eq!(block_on(stack.pop()), Ok(5));
  assert_eq!(block_on(stack.pop()), Err(StackError::Closed));
}

#[test]
fn push_blocks_until_space_available() {
  let shared = make_shared_stack(1, StackOverflowPolicy::Block);
  let stack: AsyncStack<i32, _, _> = AsyncStack::new(shared);

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
  let stack: AsyncStack<i32, _, _> = AsyncStack::new(shared);

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
  let stack: AsyncStack<i32, _, _> = AsyncStack::new(shared);

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
  let stack: AsyncStack<i32, _, _> = AsyncStack::new(shared);

  assert_eq!(block_on(stack.push(1)), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.pop()), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.peek()), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.close()), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.len()), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.capacity()), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.is_empty()), Err(StackError::WouldBlock));
  assert_eq!(block_on(stack.is_full()), Err(StackError::WouldBlock));
}
