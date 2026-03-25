use alloc::{boxed::Box, vec::Vec};
use core::{
  future::{Future, ready},
  pin::Pin,
  sync::atomic::{AtomicUsize, Ordering},
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
  time::Duration,
};

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex, SharedAccess};

use super::{graceful_stop, graceful_stop_with_message, retry};
use crate::core::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_ref::{ActorRef, ActorRefSender},
  },
  error::{ActorError, SendError},
  messaging::{AnyMessage, AnyMessageView, AskError},
  props::Props,
  scheduler::{ExecutionBatch, SchedulerCommand, SchedulerRunnable},
  system::ActorSystem,
};

struct ReplyingSender {
  replies: ArcShared<NoStdMutex<Vec<u32>>>,
}

impl ActorRefSender for ReplyingSender {
  fn send(&mut self, message: AnyMessage) -> Result<crate::core::actor::actor_ref::SendOutcome, SendError> {
    if let Some(mut sender) = message.sender().cloned() {
      sender.tell(AnyMessage::new(7_u32));
      self.replies.lock().push(7);
    }
    Ok(crate::core::actor::actor_ref::SendOutcome::Delivered)
  }
}

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct SilentSender;

impl ActorRefSender for SilentSender {
  fn send(&mut self, _message: AnyMessage) -> Result<crate::core::actor::actor_ref::SendOutcome, SendError> {
    Ok(crate::core::actor::actor_ref::SendOutcome::Delivered)
  }
}

struct DisappearingSender {
  pid:    Pid,
  system: crate::core::system::state::SystemStateShared,
}

impl ActorRefSender for DisappearingSender {
  fn send(&mut self, message: AnyMessage) -> Result<crate::core::actor::actor_ref::SendOutcome, SendError> {
    self.system.remove_cell(&self.pid);
    Err(SendError::closed(message))
  }
}

struct RemoveCellRunnable {
  pid:    Pid,
  system: crate::core::system::state::SystemStateShared,
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

#[test]
fn ask_with_timeout_completes_with_timeout_after_scheduler_tick() {
  let system = ActorSystem::new_empty().state();
  let replies = ArcShared::new(NoStdMutex::new(Vec::new()));
  let mut actor = ActorRef::with_system(Pid::new(40, 0), ReplyingSender { replies }, &system);

  let response = actor.ask_with_timeout(AnyMessage::new("ping"), Duration::from_millis(1));
  let result = response.future().with_write(|inner| inner.try_take()).expect("reply result");
  let reply = result.expect("successful reply");
  assert_eq!(reply.payload().downcast_ref::<u32>(), Some(&7_u32));
}

#[test]
fn ask_with_timeout_without_system_times_out_immediately() {
  let mut actor = ActorRef::new(Pid::new(41, 0), SilentSender);

  let response = actor.ask_with_timeout(AnyMessage::new("ping"), Duration::from_millis(1));

  let result = response.future().with_write(|inner| inner.try_take()).expect("timeout result");
  assert!(matches!(result, Err(AskError::Timeout)));
}

#[test]
fn ask_with_timeout_times_out_after_scheduler_tick() {
  let system = ActorSystem::new_empty();
  let state = system.state();
  let pid = state.allocate_pid();
  let props = Props::from_fn(|| NoopActor);
  let cell = ActorCell::create(state.clone(), pid, None, "ask-timeout".into(), &props).expect("create actor");
  state.register_cell(cell.clone());

  let response = cell.actor_ref().ask_with_timeout(AnyMessage::new("ping"), Duration::from_millis(1));
  assert!(response.future().with_read(|inner| !inner.is_ready()));

  state.scheduler().with_write(|scheduler| scheduler.run_for_test(1));

  let result = response.future().with_write(|inner| inner.try_take()).expect("timeout result");
  assert!(matches!(result, Err(AskError::Timeout)));
}

#[test]
fn child_ref_ask_with_timeout_times_out_after_scheduler_tick() {
  let system = ActorSystem::new_empty();
  let state = system.state();
  let parent_pid = state.allocate_pid();
  let parent_props = Props::from_fn(|| NoopActor);
  let parent_cell =
    ActorCell::create(state.clone(), parent_pid, None, "parent".into(), &parent_props).expect("create parent");
  state.register_cell(parent_cell);

  let mut context = ActorContext::new(&system, parent_pid);
  let child_props = Props::from_fn(|| NoopActor);
  let mut child = context.spawn_child(&child_props).expect("spawn child");

  let response = child.ask_with_timeout(AnyMessage::new("ping"), Duration::from_millis(1));
  assert!(response.future().with_read(|inner| !inner.is_ready()));

  state.scheduler().with_write(|scheduler| scheduler.run_for_test(1));

  let result = response.future().with_write(|inner| inner.try_take()).expect("timeout result");
  assert!(matches!(result, Err(AskError::Timeout)));
}

#[test]
fn graceful_stop_finishes_after_target_disappears() {
  let system = ActorSystem::new_empty();
  let state = system.state();
  let pid = state.allocate_pid();
  let props = Props::from_fn(|| NoopActor);
  let cell = ActorCell::create(state.clone(), pid, None, "graceful".into(), &props).expect("create actor");
  state.register_cell(cell.clone());

  let runnable: ArcShared<dyn SchedulerRunnable> = ArcShared::new(RemoveCellRunnable { pid, system: state.clone() });
  state.scheduler().with_write(|scheduler| {
    scheduler
      .schedule_command(Duration::from_millis(1), SchedulerCommand::RunRunnable { runnable, dispatcher: None })
      .expect("schedule removal");
  });

  let mut actor_ref = cell.actor_ref();
  let mut future = Box::pin(graceful_stop(&mut actor_ref, Duration::from_millis(5)));
  match poll_future(future.as_mut()) {
    | Poll::Ready(Ok(())) => {},
    | Poll::Pending => {
      state.scheduler().with_write(|scheduler| scheduler.run_for_test(1));
      assert!(matches!(poll_future(future.as_mut()), Poll::Ready(Ok(()))));
    },
    | other => panic!("unexpected graceful_stop result: {other:?}"),
  }
}

#[test]
fn graceful_stop_with_message_returns_timeout_when_target_stays_alive() {
  let system = ActorSystem::new_empty();
  let state = system.state();
  let pid = state.allocate_pid();
  let props = Props::from_fn(|| NoopActor);
  let cell = ActorCell::create(state.clone(), pid, None, "stubborn".into(), &props).expect("create actor");
  state.register_cell(cell.clone());

  let mut actor_ref = cell.actor_ref();
  let mut future =
    Box::pin(graceful_stop_with_message(&mut actor_ref, AnyMessage::new("stop"), Duration::from_millis(1)));
  assert!(matches!(poll_future(future.as_mut()), Poll::Pending));

  state.scheduler().with_write(|scheduler| scheduler.run_for_test(1));

  assert!(matches!(poll_future(future.as_mut()), Poll::Ready(Err(AskError::Timeout))));
}

#[test]
fn graceful_stop_returns_send_failed_without_system() {
  let mut actor_ref = ActorRef::new(Pid::new(90, 0), SilentSender);
  let mut future = Box::pin(graceful_stop(&mut actor_ref, Duration::from_millis(1)));

  assert!(matches!(poll_future(future.as_mut()), Poll::Ready(Err(AskError::SendFailed(_)))));
}

#[test]
fn graceful_stop_succeeds_when_target_is_already_terminated() {
  let system = ActorSystem::new_empty().state();
  let mut actor_ref = ActorRef::with_system(Pid::new(91, 0), SilentSender, &system);
  let mut future = Box::pin(graceful_stop(&mut actor_ref, Duration::from_millis(1)));

  assert!(matches!(poll_future(future.as_mut()), Poll::Ready(Ok(()))));
}

#[test]
fn graceful_stop_enters_poll_loop_when_stop_message_is_silently_dropped() {
  let system = ActorSystem::new_empty();
  let state = system.state();
  let pid = state.allocate_pid();
  let props = Props::from_fn(|| NoopActor);
  let cell = ActorCell::create(state.clone(), pid, None, "send-failed".into(), &props).expect("create actor");
  state.register_cell(cell);

  // stop message の同期送信が失敗しても、graceful_stop 自体はただちには
  // 失敗せず、停止観測のための poll loop に入る。
  let mut actor_ref = ActorRef::with_system(pid, crate::core::actor::actor_ref::NullSender, &state);
  let mut future = Box::pin(graceful_stop(&mut actor_ref, Duration::from_millis(1)));

  assert!(matches!(poll_future(future.as_mut()), Poll::Pending));
}

#[test]
fn graceful_stop_succeeds_when_target_disappears_during_send() {
  let system = ActorSystem::new_empty();
  let state = system.state();
  let pid = state.allocate_pid();
  let props = Props::from_fn(|| NoopActor);
  let cell = ActorCell::create(state.clone(), pid, None, "disappearing".into(), &props).expect("create actor");
  state.register_cell(cell);

  let mut actor_ref = ActorRef::with_system(pid, DisappearingSender { pid, system: state.clone() }, &state);
  let mut future = Box::pin(graceful_stop(&mut actor_ref, Duration::from_millis(1)));

  assert!(matches!(poll_future(future.as_mut()), Poll::Ready(Ok(()))));
}

#[test]
fn retry_returns_success_after_intermediate_failure() {
  let system = ActorSystem::new_empty();
  let mut delay_provider = system.delay_provider();
  let attempts = ArcShared::new(AtomicUsize::new(0));
  let attempts_for_op = attempts.clone();

  let mut future = Box::pin(retry(
    3,
    &mut delay_provider,
    |_| Duration::from_millis(1),
    move || {
      let current = attempts_for_op.fetch_add(1, Ordering::SeqCst);
      if current == 0 { ready(Err::<u32, u32>(1)) } else { ready(Ok::<u32, u32>(7)) }
    },
  ));

  assert!(matches!(poll_future(future.as_mut()), Poll::Pending));
  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));
  assert!(matches!(poll_future(future.as_mut()), Poll::Ready(Ok(7))));
}

#[test]
fn retry_returns_last_error_after_attempts_exhausted() {
  let system = ActorSystem::new_empty();
  let mut delay_provider = system.delay_provider();
  let attempts = ArcShared::new(AtomicUsize::new(0));
  let attempts_for_op = attempts.clone();

  let mut future = Box::pin(retry(
    2,
    &mut delay_provider,
    |_| Duration::from_millis(1),
    move || {
      let value = attempts_for_op.fetch_add(1, Ordering::SeqCst) as u32;
      ready(Err::<u32, u32>(value + 1))
    },
  ));

  assert!(matches!(poll_future(future.as_mut()), Poll::Pending));
  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));
  assert!(matches!(poll_future(future.as_mut()), Poll::Ready(Err(2))));
}
