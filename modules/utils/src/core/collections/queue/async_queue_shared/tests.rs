use core::{
  future::Future,
  pin::Pin,
  ptr,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use super::{AsyncMpscQueueShared, AsyncQueueShared, AsyncSpscQueueShared};
use crate::core::{
  collections::queue::{
    AsyncQueue, QueueError,
    backend::{OfferOutcome, OverflowPolicy, SyncQueueAsyncAdapter, VecDequeBackend},
    type_keys::{MpscKey, SpscKey},
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

type SpscSharedQueue =
  ArcShared<SpinAsyncMutex<AsyncQueue<i32, SpscKey, SyncQueueAsyncAdapter<i32, VecDequeBackend<i32>>>>>;
type MpscSharedQueue =
  ArcShared<SpinAsyncMutex<AsyncQueue<i32, MpscKey, SyncQueueAsyncAdapter<i32, VecDequeBackend<i32>>>>>;

fn make_shared_queue(capacity: usize, policy: OverflowPolicy) -> SpscSharedQueue {
  let backend = VecDequeBackend::with_capacity(capacity, policy);
  let async_queue = AsyncQueue::new_spsc(SyncQueueAsyncAdapter::new(backend));
  ArcShared::new(SpinAsyncMutex::new(async_queue))
}

fn make_shared_queue_mpsc(capacity: usize, policy: OverflowPolicy) -> MpscSharedQueue {
  let backend = VecDequeBackend::with_capacity(capacity, policy);
  let async_queue = AsyncQueue::new_mpsc(SyncQueueAsyncAdapter::new(backend));
  ArcShared::new(SpinAsyncMutex::new(async_queue))
}

struct DenyPolicy;

impl InterruptContextPolicy for DenyPolicy {
  fn check_blocking_allowed() -> Result<(), SharedError> {
    Err(SharedError::InterruptContext)
  }
}

type DenyMutex<T> = SpinAsyncMutex<T, DenyPolicy>;
type DenySharedQueue = ArcShared<DenyMutex<AsyncQueue<i32, SpscKey, SyncQueueAsyncAdapter<i32, VecDequeBackend<i32>>>>>;

fn make_interrupt_shared_queue(capacity: usize) -> DenySharedQueue {
  let backend = VecDequeBackend::with_capacity(capacity, OverflowPolicy::Block);
  let async_queue = AsyncQueue::new_spsc(SyncQueueAsyncAdapter::new(backend));
  ArcShared::new(DenyMutex::new(async_queue))
}

#[test]
fn offer_and_poll_operates_async_queue() {
  let shared = make_shared_queue(4, OverflowPolicy::Block);
  let queue: AsyncSpscQueueShared<i32, _, _> = AsyncQueueShared::new_spsc(shared);

  assert_eq!(block_on(queue.is_empty()), Ok(true));
  assert!(matches!(block_on(queue.offer(42)), Ok(OfferOutcome::Enqueued)));
  assert_eq!(block_on(queue.len()), Ok(1));
  assert_eq!(block_on(queue.poll()), Ok(42));
  assert_eq!(block_on(queue.is_empty()), Ok(true));
}

#[test]
fn into_mpsc_pair_roundtrip() {
  let shared = make_shared_queue_mpsc(4, OverflowPolicy::Block);
  let queue: AsyncMpscQueueShared<i32, _, _> = AsyncQueueShared::new_mpsc(shared);
  let (producer, consumer) = queue.into_mpsc_pair();

  assert!(matches!(block_on(producer.offer(7)), Ok(OfferOutcome::Enqueued)));
  assert_eq!(block_on(consumer.poll()), Ok(7));
}

#[test]
fn into_spsc_pair_roundtrip() {
  let shared = make_shared_queue(4, OverflowPolicy::Block);
  let queue: AsyncSpscQueueShared<i32, _, _> = AsyncQueueShared::new_spsc(shared);
  let (producer, consumer) = queue.into_spsc_pair();

  assert!(matches!(block_on(producer.offer(10)), Ok(OfferOutcome::Enqueued)));
  assert!(matches!(block_on(producer.offer(20)), Ok(OfferOutcome::Enqueued)));
  assert_eq!(block_on(consumer.poll()), Ok(10));
  assert_eq!(block_on(consumer.poll()), Ok(20));
}

#[test]
fn spsc_consumer_close() {
  let shared = make_shared_queue(2, OverflowPolicy::Block);
  let queue: AsyncSpscQueueShared<i32, _, _> = AsyncQueueShared::new_spsc(shared);
  let (producer, consumer) = queue.into_spsc_pair();

  assert!(matches!(block_on(producer.offer(1)), Ok(OfferOutcome::Enqueued)));
  assert!(block_on(consumer.close()).is_ok());
  assert_eq!(block_on(consumer.poll()), Ok(1));
  assert_eq!(block_on(consumer.poll()), Err(QueueError::Disconnected));
}

#[test]
fn close_prevents_further_operations() {
  let shared = make_shared_queue(2, OverflowPolicy::Block);
  let queue: AsyncSpscQueueShared<i32, _, _> = AsyncQueueShared::new_spsc(shared);

  assert!(matches!(block_on(queue.offer(1)), Ok(OfferOutcome::Enqueued)));
  assert!(block_on(queue.close()).is_ok());
  assert_eq!(block_on(queue.poll()), Ok(1));
  assert_eq!(block_on(queue.poll()), Err(QueueError::Disconnected));
  assert_eq!(block_on(queue.offer(2)), Err(QueueError::Closed(2)));
}

#[test]
fn offer_blocks_until_space_available() {
  let shared = make_shared_queue(1, OverflowPolicy::Block);
  let queue: AsyncSpscQueueShared<i32, _, _> = AsyncQueueShared::new_spsc(shared);

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
  let queue: AsyncSpscQueueShared<i32, _, _> = AsyncQueueShared::new_spsc(shared);

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
  let queue: AsyncSpscQueueShared<i32, _, _> = AsyncQueueShared::new_spsc(shared);

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
  let queue: AsyncSpscQueueShared<i32, _, _> = AsyncQueueShared::new_spsc(shared);

  assert_eq!(block_on(queue.offer(1)), Err(QueueError::WouldBlock));
  assert_eq!(block_on(queue.poll()), Err(QueueError::WouldBlock));
  assert_eq!(block_on(queue.close()), Err(QueueError::WouldBlock));
  assert_eq!(block_on(queue.len()), Err(QueueError::WouldBlock));
  assert_eq!(block_on(queue.capacity()), Err(QueueError::WouldBlock));
  assert_eq!(block_on(queue.is_empty()), Err(QueueError::WouldBlock));
  assert_eq!(block_on(queue.is_full()), Err(QueueError::WouldBlock));
}
