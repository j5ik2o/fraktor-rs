use alloc::{boxed::Box, sync::Arc};
use core::{
  num::NonZeroUsize,
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_rs::core::sync::ArcShared;

use super::dispatcher_waker;
use crate::core::kernel::dispatch::{
  dispatcher::{
    DefaultDispatcher, DispatcherSettings, ExecuteError, Executor, ExecutorShared, MessageDispatcherShared,
  },
  mailbox::{Mailbox, MailboxPolicy},
};

struct CountingExecutor {
  count: Arc<AtomicUsize>,
}

impl Executor for CountingExecutor {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
    self.count.fetch_add(1, Ordering::SeqCst);
    task();
    Ok(())
  }

  fn shutdown(&mut self) {}
}

fn nz(value: usize) -> NonZeroUsize {
  NonZeroUsize::new(value).expect("non-zero")
}

fn make_shared() -> (MessageDispatcherShared, ArcShared<Mailbox>, Arc<AtomicUsize>) {
  let count = Arc::new(AtomicUsize::new(0));
  let executor = ExecutorShared::new(CountingExecutor { count: Arc::clone(&count) });
  let settings = DispatcherSettings::new("waker", nz(5), None, Duration::from_secs(1));
  let dispatcher = DefaultDispatcher::new(&settings, executor);
  let shared = MessageDispatcherShared::new(dispatcher);
  let mailbox = ArcShared::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  (shared, mailbox, count)
}

#[test]
fn wake_invokes_register_for_execution() {
  let (shared, mailbox, count) = make_shared();
  let waker = dispatcher_waker(shared, mailbox);
  waker.wake();
  // wake() drains via register_for_execution -> executor.execute path.
  assert!(count.load(Ordering::SeqCst) >= 1);
}

#[test]
fn wake_by_ref_invokes_register_for_execution() {
  let (shared, mailbox, count) = make_shared();
  let waker = dispatcher_waker(shared, mailbox);
  waker.wake_by_ref();
  assert!(count.load(Ordering::SeqCst) >= 1);
}

#[test]
fn clone_waker_uses_independent_state() {
  let (shared, mailbox, count) = make_shared();
  let waker = dispatcher_waker(shared, mailbox);
  let cloned = waker.clone();
  drop(waker);
  cloned.wake();
  assert!(count.load(Ordering::SeqCst) >= 1);
}
