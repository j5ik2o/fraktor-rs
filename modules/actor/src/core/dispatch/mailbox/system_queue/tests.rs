use crate::core::{actor::Pid, dispatch::mailbox::system_queue::SystemQueue, messaging::system_message::SystemMessage};

#[test]
fn fifo_ordering_is_preserved() {
  let queue = SystemQueue::new();
  queue.push(SystemMessage::Suspend);
  queue.push(SystemMessage::Resume);
  queue.push(SystemMessage::Stop);

  assert!(matches!(queue.pop(), Some(SystemMessage::Suspend)));
  assert!(matches!(queue.pop(), Some(SystemMessage::Resume)));
  assert!(matches!(queue.pop(), Some(SystemMessage::Stop)));
  assert!(queue.pop().is_none());
}

#[test]
fn len_tracks_push_and_pop_operations() {
  let queue = SystemQueue::new();
  assert_eq!(queue.len(), 0);

  queue.push(SystemMessage::Watch(Pid::new(1, 0)));
  assert_eq!(queue.len(), 1);
  queue.push(SystemMessage::Unwatch(Pid::new(2, 0)));
  assert_eq!(queue.len(), 2);

  queue.pop();
  assert_eq!(queue.len(), 1);
  queue.pop();
  assert_eq!(queue.len(), 0);
}

/// Regression test for GitHub issue #126: concurrent pushers + poppers must
/// preserve per-producer FIFO order. The old `return_to_head()` broke this by
/// reinserting the reversed chain one node at a time, interleaving with new pushes.
#[test]
fn concurrent_push_pop_preserves_per_producer_fifo() {
  use core::sync::atomic::{AtomicBool, Ordering};
  use std::{
    sync::{Arc, Barrier},
    thread,
  };

  const PRODUCERS: u64 = 4;
  const ITEMS_PER_PRODUCER: u64 = 200;
  const TOTAL: usize = (PRODUCERS * ITEMS_PER_PRODUCER) as usize;
  const CONSUMERS: usize = 2;
  const ROUNDS: usize = 50;

  for _ in 0..ROUNDS {
    let queue = Arc::new(SystemQueue::new());
    let done = Arc::new(AtomicBool::new(false));
    let barrier = Arc::new(Barrier::new((PRODUCERS as usize) + CONSUMERS));

    let mut producer_handles = Vec::new();
    for producer in 0..PRODUCERS {
      let q = Arc::clone(&queue);
      let b = Arc::clone(&barrier);
      producer_handles.push(thread::spawn(move || {
        b.wait();
        let base = producer * ITEMS_PER_PRODUCER;
        for seq in 0..ITEMS_PER_PRODUCER {
          q.push(SystemMessage::Watch(Pid::new(base + seq, 0)));
        }
      }));
    }

    let mut consumer_handles = Vec::new();
    for _ in 0..CONSUMERS {
      let q = Arc::clone(&queue);
      let b = Arc::clone(&barrier);
      let d = Arc::clone(&done);
      consumer_handles.push(thread::spawn(move || {
        b.wait();
        let mut collected = Vec::new();
        loop {
          match q.pop() {
            | Some(SystemMessage::Watch(pid)) => collected.push(pid.value()),
            | Some(_) => panic!("unexpected message variant"),
            | None if d.load(Ordering::Acquire) => break,
            | None => thread::yield_now(),
          }
        }
        collected
      }));
    }

    for h in producer_handles {
      h.join().unwrap();
    }
    done.store(true, Ordering::Release);

    let consumer_results: Vec<Vec<u64>> = consumer_handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Verify per-producer FIFO: within each consumer's output,
    // items from the same producer must appear in push order.
    for (cidx, seq) in consumer_results.iter().enumerate() {
      for producer in 0..PRODUCERS {
        let base = producer * ITEMS_PER_PRODUCER;
        let end = base + ITEMS_PER_PRODUCER;
        let producer_items: Vec<u64> = seq.iter().copied().filter(|&v| v >= base && v < end).collect();
        for w in producer_items.windows(2) {
          assert!(
            w[0] < w[1],
            "FIFO violated: consumer {cidx}, producer {producer}: {v0} before {v1}",
            v0 = w[0],
            v1 = w[1],
          );
        }
      }
    }

    let mut all_values: Vec<u64> = consumer_results.into_iter().flatten().collect();
    while let Some(msg) = queue.pop() {
      if let SystemMessage::Watch(pid) = msg {
        all_values.push(pid.value());
      }
    }

    all_values.sort();
    let mut expected: Vec<u64> = Vec::with_capacity(TOTAL);
    for producer in 0..PRODUCERS {
      let base = producer * ITEMS_PER_PRODUCER;
      for seq in 0..ITEMS_PER_PRODUCER {
        expected.push(base + seq);
      }
    }
    expected.sort();
    assert_eq!(all_values, expected, "all messages must be delivered exactly once");
  }
}

/// Verify per-producer FIFO: items from each producer appear in the
/// order they were pushed, even with concurrent consumers.
#[test]
fn concurrent_consumers_see_monotonic_per_producer_sequence() {
  use std::{
    sync::{Arc, Barrier},
    thread,
  };

  const ITEMS: u64 = 500;
  const CONSUMERS: usize = 4;
  const ROUNDS: usize = 50;

  for _ in 0..ROUNDS {
    let queue = Arc::new(SystemQueue::new());

    // Single producer pushes a sequential batch.
    for i in 0..ITEMS {
      queue.push(SystemMessage::Watch(Pid::new(i, 0)));
    }

    let barrier = Arc::new(Barrier::new(CONSUMERS));
    let mut handles = Vec::new();
    for _ in 0..CONSUMERS {
      let q = Arc::clone(&queue);
      let b = Arc::clone(&barrier);
      handles.push(thread::spawn(move || {
        b.wait();
        let mut collected = Vec::new();
        while let Some(msg) = q.pop() {
          if let SystemMessage::Watch(pid) = msg {
            collected.push(pid.value());
          }
        }
        collected
      }));
    }

    let per_consumer: Vec<Vec<u64>> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Each consumer's local sequence must be strictly increasing (FIFO).
    for (idx, seq) in per_consumer.iter().enumerate() {
      for w in seq.windows(2) {
        assert!(w[0] < w[1], "FIFO violated for consumer {idx}: {v0} appeared before {v1}", v0 = w[0], v1 = w[1],);
      }
    }

    // All items delivered exactly once.
    let mut all: Vec<u64> = per_consumer.into_iter().flatten().collect();
    all.sort();
    let expected: Vec<u64> = (0..ITEMS).collect();
    assert_eq!(all, expected);
  }
}
