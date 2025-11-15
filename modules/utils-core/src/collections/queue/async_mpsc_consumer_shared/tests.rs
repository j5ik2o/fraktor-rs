use core::{
  future::Future,
  pin::Pin,
  ptr,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use super::AsyncMpscConsumerShared;
use crate::{
  collections::queue::{
    async_queue_shared::AsyncQueueShared,
    backend::{OverflowPolicy, SyncQueueAsyncAdapter, VecDequeBackend},
    type_keys::MpscKey,
  },
  sync::{ArcShared, async_mutex_like::SpinAsyncMutex},
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
) -> ArcShared<SpinAsyncMutex<SyncQueueAsyncAdapter<i32, VecDequeBackend<i32>>>> {
  let backend = VecDequeBackend::with_capacity(capacity, policy);
  ArcShared::new(SpinAsyncMutex::new(SyncQueueAsyncAdapter::new(backend)))
}

#[test]
fn async_mpsc_consumer_poll() {
  let shared = make_shared_queue(4, OverflowPolicy::Block);
  let queue = AsyncQueueShared::<i32, MpscKey, _, _>::new_mpsc(shared);
  let (_producer, consumer) = queue.into_mpsc_pair();

  let queue2 = AsyncQueueShared::<i32, MpscKey, _, _>::new_mpsc(consumer.shared().clone());
  let (producer, _consumer) = queue2.into_mpsc_pair();
  block_on(producer.offer(42)).unwrap();

  let result = block_on(consumer.poll());
  assert_eq!(result.unwrap(), 42);
}

#[test]
fn async_mpsc_consumer_is_empty() {
  let shared = make_shared_queue(4, OverflowPolicy::Block);
  let consumer = AsyncMpscConsumerShared::new(shared);

  assert_eq!(block_on(consumer.is_empty()), Ok(true));
}

#[test]
fn async_mpsc_consumer_len() {
  let shared = make_shared_queue(4, OverflowPolicy::Block);
  let queue = AsyncQueueShared::<i32, MpscKey, _, _>::new_mpsc(shared);
  let (producer, consumer) = queue.into_mpsc_pair();

  block_on(producer.offer(1)).unwrap();
  block_on(producer.offer(2)).unwrap();

  assert_eq!(block_on(consumer.len()), Ok(2));
}

#[test]
fn async_mpsc_consumer_capacity() {
  let shared = make_shared_queue(10, OverflowPolicy::Block);
  let consumer = AsyncMpscConsumerShared::new(shared);

  assert_eq!(block_on(consumer.capacity()), Ok(10));
}

#[test]
fn async_mpsc_consumer_close() {
  let shared = make_shared_queue(4, OverflowPolicy::Block);
  let consumer = AsyncMpscConsumerShared::new(shared);

  assert!(block_on(consumer.close()).is_ok());
}

#[test]
fn async_mpsc_consumer_shared() {
  let shared = make_shared_queue(4, OverflowPolicy::Block);
  let queue = AsyncQueueShared::<i32, MpscKey, _, _>::new_mpsc(shared.clone());
  let (_producer, consumer) = queue.into_mpsc_pair();

  let retrieved = consumer.shared();
  let _ = retrieved;
}
