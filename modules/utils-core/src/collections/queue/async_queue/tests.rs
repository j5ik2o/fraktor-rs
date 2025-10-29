use core::{
  future::Future,
  pin::Pin,
  ptr,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use super::{AsyncMpscQueue, AsyncQueue, AsyncSpscQueue};
use crate::{
  collections::{
    queue::{
      VecRingStorage,
      backend::{OfferOutcome, OverflowPolicy, SyncAdapterQueueBackend, VecRingBackend},
    },
    queue_old::QueueError,
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

fn make_shared_queue(
  capacity: usize,
  policy: OverflowPolicy,
) -> ArcShared<SpinAsyncMutex<SyncAdapterQueueBackend<i32, VecRingBackend<i32>>>> {
  let storage = VecRingStorage::with_capacity(capacity);
  let backend = VecRingBackend::new_with_storage(storage, policy);
  ArcShared::new(SpinAsyncMutex::new(SyncAdapterQueueBackend::new(backend)))
}

struct DenyPolicy;

impl InterruptContextPolicy for DenyPolicy {
  fn check_blocking_allowed() -> Result<(), SharedError> {
    Err(SharedError::InterruptContext)
  }
}

type DenyMutex<T> = SpinAsyncMutex<T, DenyPolicy>;

fn make_interrupt_shared_queue(
  capacity: usize,
) -> ArcShared<DenyMutex<SyncAdapterQueueBackend<i32, VecRingBackend<i32>>>> {
  let storage = VecRingStorage::with_capacity(capacity);
  let backend = VecRingBackend::new_with_storage(storage, OverflowPolicy::Block);
  ArcShared::new(DenyMutex::new(SyncAdapterQueueBackend::new(backend)))
}

#[test]
fn offer_and_poll_operates_async_queue() {
  let shared = make_shared_queue(4, OverflowPolicy::Block);
  let queue: AsyncSpscQueue<i32, _, _> = AsyncQueue::new_spsc(shared);

  assert_eq!(block_on(queue.is_empty()), Ok(true));
  assert!(matches!(block_on(queue.offer(42)), Ok(OfferOutcome::Enqueued)));
  assert_eq!(block_on(queue.len()), Ok(1));
  assert_eq!(block_on(queue.poll()), Ok(42));
  assert_eq!(block_on(queue.is_empty()), Ok(true));
}

#[test]
fn into_mpsc_pair_roundtrip() {
  let shared = make_shared_queue(4, OverflowPolicy::Block);
  let queue: AsyncMpscQueue<i32, _, _> = AsyncQueue::new_mpsc(shared);
  let (producer, consumer) = queue.into_mpsc_pair();

  assert!(matches!(block_on(producer.offer(7)), Ok(OfferOutcome::Enqueued)));
  assert_eq!(block_on(consumer.poll()), Ok(7));
}

#[test]
fn close_prevents_further_operations() {
  let shared = make_shared_queue(2, OverflowPolicy::Block);
  let queue: AsyncSpscQueue<i32, _, _> = AsyncQueue::new_spsc(shared);

  assert!(matches!(block_on(queue.offer(1)), Ok(OfferOutcome::Enqueued)));
  assert!(block_on(queue.close()).is_ok());
  assert_eq!(block_on(queue.poll()), Ok(1));
  assert_eq!(block_on(queue.poll()), Err(QueueError::Disconnected));
  assert_eq!(block_on(queue.offer(2)), Err(QueueError::Closed(2)));
}

#[test]
fn offer_blocks_until_space_available() {
  let shared = make_shared_queue(1, OverflowPolicy::Block);
  let queue: AsyncSpscQueue<i32, _, _> = AsyncQueue::new_spsc(shared);

  assert!(matches!(block_on(queue.offer(1)), Ok(OfferOutcome::Enqueued)));

  let mut offer_future = queue.offer(2);
  let mut offer_future = unsafe { Pin::new_unchecked(&mut offer_future) };

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert!(matches!(offer_future.as_mut().poll(&mut context), Poll::Pending));

  assert_eq!(block_on(queue.poll()), Ok(1));

  assert!(matches!(offer_future.as_mut().poll(&mut context), Poll::Ready(Ok(OfferOutcome::Enqueued))));
}

#[test]
fn poll_blocks_until_item_available() {
  let shared = make_shared_queue(1, OverflowPolicy::Block);
  let queue: AsyncSpscQueue<i32, _, _> = AsyncQueue::new_spsc(shared);

  let mut poll_future = queue.poll();
  let mut poll_future = unsafe { Pin::new_unchecked(&mut poll_future) };

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert!(matches!(poll_future.as_mut().poll(&mut context), Poll::Pending));

  assert!(matches!(block_on(queue.offer(7)), Ok(OfferOutcome::Enqueued)));

  assert_eq!(poll_future.as_mut().poll(&mut context), Poll::Ready(Ok(7)));
}

#[test]
fn close_wakes_waiting_consumer() {
  let shared = make_shared_queue(1, OverflowPolicy::Block);
  let queue: AsyncSpscQueue<i32, _, _> = AsyncQueue::new_spsc(shared);

  let mut poll_future = queue.poll();
  let mut poll_future = unsafe { Pin::new_unchecked(&mut poll_future) };

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert!(matches!(poll_future.as_mut().poll(&mut context), Poll::Pending));

  assert!(block_on(queue.close()).is_ok());

  assert_eq!(poll_future.as_mut().poll(&mut context), Poll::Ready(Err(QueueError::Disconnected)));
}

#[test]
fn interrupt_context_returns_would_block_errors() {
  let shared = make_interrupt_shared_queue(2);
  let queue: AsyncSpscQueue<i32, _, _> = AsyncQueue::new_spsc(shared);

  assert_eq!(block_on(queue.offer(1)), Err(QueueError::WouldBlock));
  assert_eq!(block_on(queue.poll()), Err(QueueError::WouldBlock));
  assert_eq!(block_on(queue.close()), Err(QueueError::WouldBlock));
  assert_eq!(block_on(queue.len()), Err(QueueError::WouldBlock));
  assert_eq!(block_on(queue.capacity()), Err(QueueError::WouldBlock));
  assert_eq!(block_on(queue.is_empty()), Err(QueueError::WouldBlock));
  assert_eq!(block_on(queue.is_full()), Err(QueueError::WouldBlock));
}
