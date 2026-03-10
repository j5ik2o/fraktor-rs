//! Pattern API example using a deterministic tick driver.
//!
//! Demonstrates `ask_with_timeout`, `graceful_stop`, and `retry` with the
//! same `core` APIs that are available in no_std-oriented code paths.

#![cfg_attr(all(not(test), target_os = "none"), no_std)]

extern crate alloc;

#[cfg(not(target_os = "none"))]
#[path = "../no_std_tick_driver_support.rs"]
mod no_std_tick_driver_support;

use alloc::boxed::Box;
use core::{
  future::{Future, ready},
  pin::Pin,
  sync::atomic::{AtomicUsize, Ordering},
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
  time::Duration,
};
#[cfg(not(target_os = "none"))]
use std::{thread, time::Duration as StdDuration};

use fraktor_actor_rs::core::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_ref::{ActorRef, ActorRefSender},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessage, AnyMessageView},
  pattern::{graceful_stop, retry},
  props::Props,
  scheduler::{ExecutionBatch, SchedulerCommand, SchedulerRunnable},
  system::ActorSystem,
};
use fraktor_utils_rs::core::sync::{ArcShared, SharedAccess};

struct ReplyingSender;

impl ActorRefSender for ReplyingSender {
  fn send(&mut self, message: AnyMessage) -> Result<fraktor_actor_rs::core::actor::actor_ref::SendOutcome, SendError> {
    if let Some(sender) = message.sender() {
      sender.tell(AnyMessage::new(7_u32))?;
    }
    Ok(fraktor_actor_rs::core::actor::actor_ref::SendOutcome::Delivered)
  }
}

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct RemoveCellRunnable {
  pid:    Pid,
  system: fraktor_actor_rs::core::system::state::SystemStateShared,
}

impl SchedulerRunnable for RemoveCellRunnable {
  fn run(&self, _batch: &ExecutionBatch) {
    self.system.remove_cell(&self.pid);
  }
}

fn poll_future<F>(mut future: Pin<&mut F>) -> Poll<F::Output>
where
  F: Future + ?Sized, {
  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);
  future.as_mut().poll(&mut context)
}

fn noop_waker() -> Waker {
  const VTABLE: RawWakerVTable = RawWakerVTable::new(clone_waker, wake_noop, wake_by_ref_noop, drop_noop);

  unsafe fn raw_waker() -> RawWaker {
    // SAFETY: this no-op waker never dereferences the data pointer, so a null pointer is valid
    // as long as every vtable function treats it as opaque.
    RawWaker::new(core::ptr::null(), &VTABLE)
  }

  unsafe fn clone_waker(data: *const ()) -> RawWaker {
    // SAFETY: cloning preserves the same opaque pointer and shares the same no-op vtable, so
    // the cloned waker has identical behavior and ownership requirements.
    RawWaker::new(data, &VTABLE)
  }

  unsafe fn wake_noop(_data: *const ()) {}

  unsafe fn wake_by_ref_noop(_data: *const ()) {}

  unsafe fn drop_noop(_data: *const ()) {}

  // SAFETY: `raw_waker()` returns a `RawWaker` whose vtable never touches the null data pointer
  // and whose clone/drop operations preserve those invariants.
  unsafe { Waker::from_raw(raw_waker()) }
}

#[cfg(not(target_os = "none"))]
fn main() {
  fn wait_until_ready<F>(future: Pin<&mut F>) -> F::Output
  where
    F: Future + ?Sized, {
    let mut future = future;
    loop {
      match poll_future(future.as_mut()) {
        | Poll::Ready(result) => return result,
        | Poll::Pending => thread::sleep(StdDuration::from_millis(20)),
      }
    }
  }

  let props = Props::from_fn(|| NoopActor);
  let (tick_driver, _pulse_handle) = no_std_tick_driver_support::hardware_tick_driver_config();
  let system = ActorSystem::new(&props, tick_driver).expect("system");
  let state = system.state();

  let ask_actor = ActorRef::with_system(Pid::new(10, 0), ReplyingSender, &state);
  let ask_response =
    ask_actor.ask_with_timeout(AnyMessage::new("ping"), Duration::from_millis(50)).expect("ask should send");
  let ask_result = ask_response.future().with_write(|inner| inner.try_take()).expect("reply result");
  let reply = ask_result.expect("successful reply");
  assert_eq!(reply.payload().downcast_ref::<u32>(), Some(&7_u32));

  let pid = state.allocate_pid();
  let props = Props::from_fn(|| NoopActor);
  let cell = ActorCell::create(state.clone(), pid, None, "graceful-example".into(), &props).expect("create actor");
  state.register_cell(cell.clone());

  let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(RemoveCellRunnable { pid, system: state.clone() });
  state.scheduler().with_write(|scheduler| {
    scheduler
      .schedule_command(Duration::from_millis(20), SchedulerCommand::RunRunnable { runnable, dispatcher: None })
      .expect("schedule removal");
  });

  let actor_ref = cell.actor_ref();
  let mut graceful = Box::pin(graceful_stop(&actor_ref, Duration::from_millis(200)));
  wait_until_ready(graceful.as_mut()).expect("graceful stop");

  let attempts = ArcShared::new(AtomicUsize::new(0));
  let attempts_for_op = attempts.clone();
  let mut delay_provider = system.delay_provider();
  let mut retried = Box::pin(retry(
    3,
    &mut delay_provider,
    |_| Duration::from_millis(20),
    move || {
      let current = attempts_for_op.fetch_add(1, Ordering::SeqCst);
      if current == 0 { ready(Err::<u32, u32>(1)) } else { ready(Ok::<u32, u32>(7)) }
    },
  ));
  assert_eq!(wait_until_ready(retried.as_mut()), Ok(7));

  let termination = system.when_terminated();
  system.terminate().expect("terminate");
  while termination.with_read(|inner| !inner.is_ready()) {
    thread::sleep(StdDuration::from_millis(20));
  }
}

#[cfg(target_os = "none")]
fn main() {}

#[cfg(test)]
mod tests {
  use core::future::Future;

  use super::*;

  struct PendingOnce {
    polled: bool,
  }

  impl Future for PendingOnce {
    type Output = u32;

    fn poll(mut self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
      if self.polled {
        Poll::Ready(9)
      } else {
        self.polled = true;
        Poll::Pending
      }
    }
  }

  #[test]
  fn should_poll_ready_future_in_no_std_helpers() {
    let mut future = Box::pin(ready(7_u32));
    assert!(matches!(poll_future(future.as_mut()), Poll::Ready(7_u32)));
  }

  #[test]
  fn should_keep_pending_polling_std_free_in_no_std_helpers() {
    let mut future = Box::pin(PendingOnce { polled: false });
    assert!(matches!(poll_future(future.as_mut()), Poll::Pending));
    assert!(matches!(poll_future(future.as_mut()), Poll::Ready(9)));
  }
}
