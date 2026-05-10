use alloc::{sync::Arc, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::{sync::Barrier, thread};

use super::ExclusiveCell;

#[test]
fn with_write_serializes_concurrent_mutations() {
  let workers = 8;
  let iterations = 256;
  let cell = Arc::new(ExclusiveCell::new(0_usize));
  let in_flight = Arc::new(AtomicUsize::new(0));
  let max_in_flight = Arc::new(AtomicUsize::new(0));
  let barrier = Arc::new(Barrier::new(workers));

  let handles = (0..workers)
    .map(|_| {
      let cell = Arc::clone(&cell);
      let in_flight = Arc::clone(&in_flight);
      let max_in_flight = Arc::clone(&max_in_flight);
      let barrier = Arc::clone(&barrier);
      thread::spawn(move || {
        barrier.wait();
        for _ in 0..iterations {
          cell.with_write(|value| {
            let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            max_in_flight.fetch_max(current, Ordering::SeqCst);
            *value += 1;
            in_flight.fetch_sub(1, Ordering::SeqCst);
          });
        }
      })
    })
    .collect::<Vec<_>>();

  for handle in handles {
    handle.join().expect("worker should finish");
  }

  assert_eq!(cell.with_read(|value| *value), workers * iterations);
  assert_eq!(max_in_flight.load(Ordering::SeqCst), 1);
}
