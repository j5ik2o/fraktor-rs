use core::{
  future::Future,
  pin::pin,
  task::{Context, Poll, Waker},
  time::Duration,
};
use std::{thread, time::Instant};

use fraktor_actor_core_rs::core::kernel::system::{CoordinatedShutdown, CoordinatedShutdownReason};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

fn main() {
  let shutdown = CoordinatedShutdown::with_default_phases().expect("default phases");
  let events = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::<&'static str>::new());
  shutdown
    .add_task(CoordinatedShutdown::PHASE_BEFORE_ACTOR_SYSTEM_TERMINATE, "flush-example", {
      let events = events.clone();
      move || async move {
        events.with_lock(|events| events.push("flushed"));
      }
    })
    .expect("add task");

  block_on_ready(
    shutdown.run(CoordinatedShutdownReason::Custom("typed-coordinated-shutdown".into())),
    Duration::from_secs(5),
  )
  .expect("coordinated shutdown should complete before timeout");

  assert_eq!(events.with_lock(|events| events.clone()), vec!["flushed"]);
  assert_eq!(shutdown.shutdown_reason(), Some(CoordinatedShutdownReason::Custom("typed-coordinated-shutdown".into())));
  println!("typed_coordinated_shutdown ran tasks: {:?}", events.with_lock(|events| events.clone()));
}

fn block_on_ready<F: Future>(future: F, timeout: Duration) -> Option<F::Output> {
  let deadline = Instant::now() + timeout;
  let waker = Waker::noop();
  let mut context = Context::from_waker(waker);
  let mut future = pin!(future);
  loop {
    match future.as_mut().poll(&mut context) {
      | Poll::Ready(value) => return Some(value),
      | Poll::Pending => {
        if Instant::now() >= deadline {
          return None;
        }
        thread::sleep(Duration::from_millis(1));
      },
    }
  }
}
