use alloc::{boxed::Box, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_actor_core_kernel_rs::dispatch::dispatcher::{
  DEFAULT_BLOCKING_DISPATCHER_ID, DEFAULT_DISPATCHER_ID, ExecuteError, ExecutorFactory,
};

use crate::dispatch::EmbassyExecutorFactory;

#[test]
fn executor_enqueues_until_driver_drains_ready_queue() {
  let factory = EmbassyExecutorFactory::<4>::new();
  let driver = factory.driver();
  let executor = factory.create(DEFAULT_DISPATCHER_ID);
  let count = Arc::new(AtomicUsize::new(0));
  let count_clone = count.clone();

  executor
    .execute(
      Box::new(move || {
        count_clone.fetch_add(1, Ordering::SeqCst);
      }),
      0,
    )
    .expect("execute should enqueue");

  assert_eq!(count.load(Ordering::SeqCst), 0);
  assert_eq!(driver.drain_ready(), 1);
  assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn executor_returns_error_when_ready_queue_is_full() {
  let factory = EmbassyExecutorFactory::<1>::new();
  let executor = factory.create(DEFAULT_DISPATCHER_ID);

  executor.execute(Box::new(|| {}), 0).expect("first enqueue");
  let result = executor.execute(Box::new(|| {}), 0);

  assert!(matches!(result, Err(ExecuteError::Rejected)));
}

#[test]
fn executor_rejects_enqueue_after_shutdown() {
  let factory = EmbassyExecutorFactory::<1>::new();
  let executor = factory.create(DEFAULT_DISPATCHER_ID);

  executor.shutdown();
  let result = executor.execute(Box::new(|| {}), 0);

  assert!(matches!(result, Err(ExecuteError::Shutdown)));
}

#[test]
fn executor_shutdown_does_not_stop_other_executor_from_same_factory() {
  let factory = EmbassyExecutorFactory::<2>::new();
  let driver = factory.driver();
  let executor = factory.create(DEFAULT_DISPATCHER_ID);
  let other_executor = factory.create(DEFAULT_BLOCKING_DISPATCHER_ID);
  let count = Arc::new(AtomicUsize::new(0));
  let count_clone = count.clone();

  executor.shutdown();
  other_executor
    .execute(
      Box::new(move || {
        count_clone.fetch_add(1, Ordering::SeqCst);
      }),
      0,
    )
    .expect("other executor should keep accepting tasks");

  assert_eq!(driver.drain_ready(), 1);
  assert_eq!(count.load(Ordering::SeqCst), 1);
}
