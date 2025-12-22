//! Tick driver runtime unit tests.

use alloc::boxed::Box;
use core::{
  sync::atomic::{AtomicUsize, Ordering},
  time::Duration,
};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, SyncMutexFamily},
  sync::ArcShared,
};

use crate::core::scheduler::{
  TickDriverControl, TickDriverHandleGeneric, TickDriverId, TickDriverKind, TickDriverRuntime, TickExecutorSignal,
  TickFeed,
};

struct RecordingControl {
  shutdown_calls: ArcShared<AtomicUsize>,
}

impl TickDriverControl for RecordingControl {
  fn shutdown(&self) {
    self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
  }
}

fn runtime_with_executor_shutdown(
  executor_calls: ArcShared<AtomicUsize>,
  driver_calls: ArcShared<AtomicUsize>,
) -> TickDriverRuntime<NoStdToolbox> {
  let control: Box<dyn TickDriverControl> = Box::new(RecordingControl { shutdown_calls: driver_calls });
  let control = ArcShared::new(<<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(control));
  let handle =
    TickDriverHandleGeneric::new(TickDriverId::new(1), TickDriverKind::Auto, Duration::from_millis(1), control);
  let feed = TickFeed::<NoStdToolbox>::new(Duration::from_millis(1), 1, TickExecutorSignal::new());

  TickDriverRuntime::new(handle, feed).with_executor_shutdown(move || {
    executor_calls.fetch_add(1, Ordering::SeqCst);
  })
}

#[test]
fn shutdown_invokes_executor_shutdown_only_once() {
  let executor_calls = ArcShared::new(AtomicUsize::new(0));
  let driver_calls = ArcShared::new(AtomicUsize::new(0));
  let mut runtime = runtime_with_executor_shutdown(executor_calls.clone(), driver_calls.clone());

  runtime.shutdown();
  runtime.shutdown();

  assert_eq!(executor_calls.load(Ordering::SeqCst), 1);
  assert!(driver_calls.load(Ordering::SeqCst) >= 1);
}

#[test]
fn shutdown_on_clone_does_not_invoke_executor_shutdown() {
  let executor_calls = ArcShared::new(AtomicUsize::new(0));
  let driver_calls = ArcShared::new(AtomicUsize::new(0));
  let mut runtime = runtime_with_executor_shutdown(executor_calls.clone(), driver_calls.clone());

  let mut cloned = runtime.clone();
  cloned.shutdown();
  assert_eq!(executor_calls.load(Ordering::SeqCst), 0);

  runtime.shutdown();
  assert_eq!(executor_calls.load(Ordering::SeqCst), 1);
}
