use alloc::{vec, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::dispatch::mailbox::lock_free_mpsc_queue::LockFreeMpscQueue;

#[test]
fn default_constructs_empty_queue() {
  let queue = LockFreeMpscQueue::<u32>::default();

  assert_eq!(queue.len(), 0);
  assert!(queue.pop().is_none());
}

#[test]
fn single_producer_fifo_and_exact_once() {
  let queue = LockFreeMpscQueue::new();
  for value in 0..5_u32 {
    queue.push(value).expect("push succeeds");
  }

  assert_eq!(queue.len(), 5);
  let drained: Vec<u32> = (0..5).map(|_| queue.pop().expect("queued value")).collect();

  assert_eq!(drained, vec![0, 1, 2, 3, 4]);
  assert!(queue.pop().is_none());
  assert_eq!(queue.len(), 0);
}

#[test]
fn close_rejects_later_push() {
  let queue = LockFreeMpscQueue::new();
  queue.close();

  let rejected = queue.push(7_u32).expect_err("closed queue rejects");

  assert_eq!(rejected, 7);
  assert_eq!(queue.len(), 0);
  assert!(queue.pop().is_none());
}

#[test]
fn close_and_drain_removes_pending_items() {
  let queue = LockFreeMpscQueue::new();
  for value in 0..8_u32 {
    queue.push(value).expect("push succeeds");
  }

  queue.close_and_drain();

  assert_eq!(queue.len(), 0);
  assert!(queue.pop().is_none());
}

#[test]
fn close_waits_for_entered_producer_guard() {
  use std::{sync::mpsc, thread, time::Duration};

  let queue = Arc::new(LockFreeMpscQueue::<u32>::new());
  let producer_guard = queue.enter_producer().expect("producer enters before close");
  let (started_tx, started_rx) = mpsc::channel();
  let (finished_tx, finished_rx) = mpsc::channel();

  let queue_for_close = Arc::clone(&queue);
  let handle = thread::spawn(move || {
    started_tx.send(()).expect("close start signal");
    queue_for_close.close();
    finished_tx.send(()).expect("close finish signal");
  });

  started_rx.recv().expect("close starts");
  assert!(finished_rx.recv_timeout(Duration::from_millis(10)).is_err(), "close must wait for producer guard");

  drop(producer_guard);
  finished_rx.recv_timeout(Duration::from_secs(1)).expect("close finishes after producer guard drops");
  handle.join().expect("close thread joins");
}

#[test]
fn multiple_producers_exact_once_and_per_producer_fifo() {
  use std::{sync::Barrier, thread};

  const PRODUCERS: u64 = 4;
  const ITEMS_PER_PRODUCER: u64 = 200;
  const TOTAL: usize = (PRODUCERS * ITEMS_PER_PRODUCER) as usize;

  let queue = Arc::new(LockFreeMpscQueue::new());
  let barrier = Arc::new(Barrier::new(PRODUCERS as usize));
  let mut handles = Vec::new();
  for producer in 0..PRODUCERS {
    let q = Arc::clone(&queue);
    let b = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
      b.wait();
      let base = producer * ITEMS_PER_PRODUCER;
      for seq in 0..ITEMS_PER_PRODUCER {
        q.push(base + seq).expect("push succeeds");
      }
    }));
  }

  for handle in handles {
    handle.join().expect("producer joins");
  }

  let mut drained = Vec::with_capacity(TOTAL);
  while let Some(value) = queue.pop() {
    drained.push(value);
  }

  assert_eq!(drained.len(), TOTAL);
  for producer in 0..PRODUCERS {
    let base = producer * ITEMS_PER_PRODUCER;
    let end = base + ITEMS_PER_PRODUCER;
    let producer_values: Vec<u64> = drained.iter().copied().filter(|value| *value >= base && *value < end).collect();
    assert_eq!(producer_values.len(), ITEMS_PER_PRODUCER as usize);
    for window in producer_values.windows(2) {
      assert!(window[0] < window[1], "producer {producer} FIFO order must be preserved");
    }
  }

  let mut sorted = drained;
  sorted.sort_unstable();
  let expected: Vec<u64> =
    (0..PRODUCERS).flat_map(|p| (0..ITEMS_PER_PRODUCER).map(move |s| p * ITEMS_PER_PRODUCER + s)).collect();
  assert_eq!(sorted, expected);
  assert_eq!(queue.len(), 0);
}

#[test]
fn concurrent_dequeue_is_serialized_and_exact_once() {
  use std::{sync::Barrier, thread};

  const ITEMS: u64 = 500;
  const CONSUMERS: usize = 4;

  let queue = Arc::new(LockFreeMpscQueue::new());
  for value in 0..ITEMS {
    queue.push(value).expect("push succeeds");
  }

  let barrier = Arc::new(Barrier::new(CONSUMERS));
  let mut handles = Vec::new();
  for _ in 0..CONSUMERS {
    let q = Arc::clone(&queue);
    let b = Arc::clone(&barrier);
    handles.push(thread::spawn(move || {
      b.wait();
      let mut values = Vec::new();
      while let Some(value) = q.pop() {
        values.push(value);
      }
      values
    }));
  }

  let mut drained: Vec<u64> = handles.into_iter().flat_map(|handle| handle.join().expect("consumer joins")).collect();
  drained.sort_unstable();

  assert_eq!(drained, (0..ITEMS).collect::<Vec<_>>());
  assert_eq!(queue.len(), 0);
}

#[test]
fn close_racing_with_producers_leaves_no_residual_items() {
  use std::{sync::Barrier, thread};

  const PRODUCERS: usize = 4;
  const ITEMS_PER_PRODUCER: usize = 500;

  let queue = Arc::new(LockFreeMpscQueue::new());
  let barrier = Arc::new(Barrier::new(PRODUCERS + 1));
  let producers_started = Arc::new(AtomicUsize::new(0));
  let mut handles = Vec::new();
  for producer in 0..PRODUCERS {
    let q = Arc::clone(&queue);
    let b = Arc::clone(&barrier);
    let started = Arc::clone(&producers_started);
    handles.push(thread::spawn(move || {
      b.wait();
      started.fetch_add(1, Ordering::SeqCst);
      let base = producer * ITEMS_PER_PRODUCER;
      for seq in 0..ITEMS_PER_PRODUCER {
        if q.push(base + seq).is_err() {
          break;
        }
      }
    }));
  }

  barrier.wait();
  while producers_started.load(Ordering::SeqCst) == 0 {
    thread::yield_now();
  }
  queue.close_and_drain();

  for handle in handles {
    handle.join().expect("producer joins");
  }

  assert_eq!(queue.len(), 0);
  assert!(queue.pop().is_none());
  assert!(queue.push(usize::MAX).is_err());
}

#[test]
fn drop_releases_each_queued_item_once() {
  struct DropCounter {
    drops: Arc<AtomicUsize>,
  }

  impl Drop for DropCounter {
    fn drop(&mut self) {
      self.drops.fetch_add(1, Ordering::SeqCst);
    }
  }

  let drops = Arc::new(AtomicUsize::new(0));
  {
    let queue = LockFreeMpscQueue::new();
    for _ in 0..10 {
      assert!(queue.push(DropCounter { drops: Arc::clone(&drops) }).is_ok());
    }
    drop(queue.pop());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
  }

  assert_eq!(drops.load(Ordering::SeqCst), 10);
}

#[cfg(loom)]
mod loom_tests {
  use crate::dispatch::mailbox::lock_free_mpsc_queue::LockFreeMpscQueue;

  #[test]
  fn producer_guard_close_interleaving_rejects_or_drains() {
    loom::model(|| {
      use loom::{
        sync::{
          Arc,
          atomic::{AtomicUsize, Ordering},
        },
        thread,
      };

      const FIRST: usize = 0b01;
      const SECOND: usize = 0b10;

      let queue = Arc::new(LockFreeMpscQueue::new());
      let accepted = Arc::new(AtomicUsize::new(0));

      let producer_queue = Arc::clone(&queue);
      let producer_accepted = Arc::clone(&accepted);
      let producer = thread::spawn(move || {
        if producer_queue.push(FIRST).is_ok() {
          producer_accepted.fetch_or(FIRST, Ordering::SeqCst);
        }

        thread::yield_now();

        if producer_queue.push(SECOND).is_ok() {
          producer_accepted.fetch_or(SECOND, Ordering::SeqCst);
        }
      });

      let closer_queue = Arc::clone(&queue);
      let closer = thread::spawn(move || {
        thread::yield_now();
        closer_queue.close();
      });

      producer.join().expect("producer joins");
      closer.join().expect("closer joins");

      let mut drained_mask = 0;
      let mut drained_count = 0;
      while let Some(value) = queue.pop() {
        drained_mask |= value;
        drained_count += 1;
      }

      let accepted_mask = accepted.load(Ordering::SeqCst);
      assert_eq!(drained_mask, accepted_mask);
      assert_eq!(drained_count, accepted_mask.count_ones() as usize);
      assert!(queue.push(0b100).is_err());
    });
  }
}
