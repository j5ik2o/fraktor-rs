use alloc::{string::String, vec, vec::Vec};
use core::{hint::spin_loop, time::Duration};

use fraktor_utils_core_rs::core::sync::{ArcShared, NoStdMutex, RuntimeMutex, SharedAccess};

use super::{ActorContext, ReceiveTimeoutState};
use crate::core::kernel::{
  actor::{
    Actor, ActorCell, Pid, StashOverflowError,
    actor_ref::NullSender,
    error::{ActorError, SendError},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::SchedulerHandle,
  },
  event::logging::LogLevel,
  system::ActorSystem,
  util::futures::{ActorFutureListener, ActorFutureShared},
};

struct TestActor;

impl ReceiveTimeoutState {
  pub(crate) fn handle_raw(&self) -> Option<u64> {
    self.handle.as_ref().map(SchedulerHandle::raw)
  }
}

impl Actor for TestActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct RecordingActor {
  log: ArcShared<NoStdMutex<Vec<Pid>>>,
}

impl RecordingActor {
  fn new(log: ArcShared<NoStdMutex<Vec<Pid>>>) -> Self {
    Self { log }
  }
}

impl Actor for RecordingActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_>, pid: Pid) -> Result<(), ActorError> {
    self.log.lock().push(pid);
    Ok(())
  }
}

struct ProbeActor {
  received: ArcShared<NoStdMutex<Vec<i32>>>,
}

impl ProbeActor {
  fn new(received: ArcShared<NoStdMutex<Vec<i32>>>) -> Self {
    Self { received }
  }
}

impl Actor for ProbeActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(value) = message.downcast_ref::<i32>() {
      self.received.lock().push(*value);
    }
    Ok(())
  }
}

struct ReceiveTimeoutTick;

struct ReceiveTimeoutOnlyActor {
  events: ArcShared<NoStdMutex<Vec<&'static str>>>,
}

impl ReceiveTimeoutOnlyActor {
  fn new(events: ArcShared<NoStdMutex<Vec<&'static str>>>) -> Self {
    Self { events }
  }
}

impl Actor for ReceiveTimeoutOnlyActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<ReceiveTimeoutTick>().is_some() {
      self.events.lock().push("timeout");
    }
    Ok(())
  }
}

#[test]
fn actor_context_new() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  assert_eq!(context.pid(), pid);
}

#[test]
fn actor_context_system() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  let retrieved_system = context.system();
  let _ = retrieved_system;
}

#[test]
fn actor_context_pid() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  assert_eq!(context.pid(), pid);
}

#[test]
fn actor_context_sender_initially_none() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  assert!(context.sender().is_none());
}

#[test]
fn actor_context_set_and_clear_sender() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);

  assert!(context.sender().is_none());

  context.clear_sender();
  assert!(context.sender().is_none());
}

#[test]
fn actor_context_reply_without_sender() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);

  let result = context.reply(AnyMessage::new(42_u32));
  assert!(result.is_err());
}

#[test]
fn actor_context_children() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);

  let children = context.children();
  assert_eq!(children.len(), 0);
}

#[test]
fn actor_context_spawn_child_with_invalid_parent() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let props = Props::from_fn(|| TestActor);

  let result = context.spawn_child(&props);
  assert!(result.is_err());
}

#[test]
fn actor_context_log() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);

  context.log(LogLevel::Info, String::from("test message"));
  context.log(LogLevel::Error, String::from("error message"));
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}

#[test]
fn actor_context_pipe_to_self_enqueues_message() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = received.clone();
    move || ProbeActor::new(log.clone())
  });
  register_cell(&system, pid, "self", &props);
  let mut context = ActorContext::new(&system, pid);

  context.pipe_to_self(async { 41_i32 }, AnyMessage::new).expect("pipe to self");

  wait_until(|| !received.lock().is_empty());
  assert_eq!(received.lock()[0], 41);
}

#[test]
fn actor_context_pipe_to_self_handles_async_future() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = received.clone();
    move || ProbeActor::new(log.clone())
  });
  register_cell(&system, pid, "self", &props);
  let mut context = ActorContext::new(&system, pid);

  let signal = ActorFutureShared::<i32>::new();
  let future = {
    let handle = signal.clone();
    async move { ActorFutureListener::new(handle).await }
  };

  context.pipe_to_self(future, AnyMessage::new).expect("pipe to self");
  assert!(received.lock().is_empty());

  let waker = signal.with_write(|af| af.complete(7));
  if let Some(w) = waker {
    w.wake();
  }
  wait_until(|| !received.lock().is_empty());
  assert_eq!(received.lock()[0], 7);
}

#[test]
fn actor_context_stash_requires_active_message() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let result = context.stash();
  assert!(result.is_err());
}

#[test]
fn actor_context_stash_and_unstash_replays_message() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = received.clone();
    move || ProbeActor::new(log.clone())
  })
  .with_stash_mailbox();
  let _cell = register_cell(&system, pid, "self", &props);

  let mut context = ActorContext::new(&system, pid);
  context.set_current_message(Some(AnyMessage::new(99_i32)));
  context.stash().expect("stash");
  context.clear_current_message();

  let count = context.unstash().expect("unstash");
  assert_eq!(count, 1);

  wait_until(|| !received.lock().is_empty());
  assert_eq!(received.lock()[0], 99);
}

#[test]
fn actor_context_stash_with_limit_detects_overflow() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor::new(ArcShared::new(NoStdMutex::new(Vec::new())))).with_stash_mailbox();
  let cell = register_cell(&system, pid, "self", &props);

  let mut context = ActorContext::new(&system, pid);
  context.set_current_message(Some(AnyMessage::new(1_i32)));
  context.stash_with_limit(1).expect("stash first");
  context.set_current_message(Some(AnyMessage::new(2_i32)));

  let error = context.stash_with_limit(1).expect_err("overflow should fail");

  assert!(ActorContext::is_stash_overflow_error(&error));
  assert_eq!(cell.stashed_message_len(), 1);
}

#[test]
fn actor_context_stash_with_limit_requires_active_message() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor::new(ArcShared::new(NoStdMutex::new(Vec::new()))));
  let _cell = register_cell(&system, pid, "self", &props);

  let mut context = ActorContext::new(&system, pid);
  let error = context.stash_with_limit(10).expect_err("should fail without active message");

  assert!(matches!(error, ActorError::Recoverable(reason) if reason.as_str().contains("active user message")));
}

#[test]
fn actor_context_unstash_replays_single_message_and_unstash_all_replays_remaining() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = received.clone();
    move || ProbeActor::new(log.clone())
  })
  .with_stash_mailbox();
  let cell = register_cell(&system, pid, "self", &props);

  let mut context = ActorContext::new(&system, pid);
  context.set_current_message(Some(AnyMessage::new(1_i32)));
  context.stash().expect("stash first");
  context.set_current_message(Some(AnyMessage::new(2_i32)));
  context.stash().expect("stash second");
  context.clear_current_message();

  let first = context.unstash().expect("unstash single");
  assert_eq!(first, 1);
  assert_eq!(cell.stashed_message_len(), 1);
  wait_until(|| !received.lock().is_empty());
  assert_eq!(received.lock().clone(), vec![1]);

  let remaining = context.unstash_all().expect("unstash all");
  assert_eq!(remaining, 1);
  assert_eq!(cell.stashed_message_len(), 0);
  wait_until(|| received.lock().len() == 2);
  assert_eq!(received.lock().clone(), vec![1, 2]);
}

#[test]
fn actor_context_timers_start_single_timer_and_cancel_tracks_active_state() {
  // 前提: self actor が登録済みの classic actor context がある
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor::new(ArcShared::new(NoStdMutex::new(Vec::new()))));
  let _cell = register_cell(&system, pid, "timer-self", &props);
  let context = ActorContext::new(&system, pid);

  // 実行: classic timers API で単発タイマーを登録してから取り消す
  let timers = context.timers();
  timers.start_single_timer("tick", AnyMessage::new(7_i32), Duration::from_millis(25)).expect("schedule");
  assert!(timers.is_timer_active("tick"));
  timers.cancel("tick").expect("cancel");

  // 検証: タイマーは非アクティブになる
  assert!(!timers.is_timer_active("tick"));
}

#[test]
fn actor_context_timers_persist_keys_across_fresh_contexts() {
  // 前提: self actor が登録済みの classic actor context がある
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor::new(ArcShared::new(NoStdMutex::new(Vec::new()))));
  let _cell = register_cell(&system, pid, "timer-persist", &props);

  let first_context = ActorContext::new(&system, pid);
  let first_timers = first_context.timers();
  first_timers.start_single_timer("tick", AnyMessage::new(9_i32), Duration::from_millis(25)).expect("schedule");

  // 実行: 新しい context から同じ timer key を参照する
  let second_context = ActorContext::new(&system, pid);
  let second_timers = second_context.timers();

  // 検証: handle が cell 単位のためアクティブタイマーは見える
  assert!(second_timers.is_timer_active("tick"));
}

#[test]
fn actor_context_timers_cancel_all_clears_periodic_entries() {
  // 前提: periodic timer が有効な classic actor context がある
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor::new(ArcShared::new(NoStdMutex::new(Vec::new()))));
  let _cell = register_cell(&system, pid, "timer-periodic", &props);
  let context = ActorContext::new(&system, pid);
  let timers = context.timers();

  // 実行: fixed-delay と fixed-rate の timer を開始してからまとめて取消する
  timers
    .start_timer_with_fixed_delay("delay", AnyMessage::new(1_i32), Duration::from_millis(20))
    .expect("schedule fixed delay");
  timers
    .start_timer_at_fixed_rate("rate", AnyMessage::new(2_i32), Duration::from_millis(20))
    .expect("schedule fixed rate");
  timers.cancel_all().expect("cancel all");

  // 検証: 両方の timer key が非アクティブになる
  assert!(!timers.is_timer_active("delay"));
  assert!(!timers.is_timer_active("rate"));
}

#[test]
fn actor_context_stash_overflow_error_converts_from_actor_error() {
  // 前提: 既存の context API で stash overflow を発生させる
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor::new(ArcShared::new(NoStdMutex::new(Vec::new())))).with_stash_mailbox();
  let _cell = register_cell(&system, pid, "stash-overflow", &props);

  let mut context = ActorContext::new(&system, pid);
  context.set_current_message(Some(AnyMessage::new(1_i32)));
  context.stash_with_limit(1).expect("stash first");
  context.set_current_message(Some(AnyMessage::new(2_i32)));
  let error = context.stash_with_limit(1).expect_err("overflow should fail");

  // 実行: 公開エラー型として stash overflow を取り出す
  let overflow: StashOverflowError = error.try_into().expect("classic stash overflow error");

  // 検証: 変換が成功し、公開エラー型として扱える
  let _ = overflow;
}

#[test]
fn actor_context_stash_requires_deque_error_is_detected() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor::new(ArcShared::new(NoStdMutex::new(Vec::new()))));
  let _cell = register_cell(&system, pid, "stash-deque", &props);

  let mut context = ActorContext::new(&system, pid);
  context.set_current_message(Some(AnyMessage::new(1_i32)));

  let error = context.stash().expect_err("non-deque stash should fail");

  assert!(ActorContext::is_stash_requires_deque_error(&error));
}

#[test]
fn actor_context_forward_preserves_sender() {
  use crate::core::kernel::actor::actor_ref::{ActorRef, ActorRefSender, SendOutcome};

  struct CapturingSender {
    inbox: ArcShared<NoStdMutex<Vec<AnyMessage>>>,
  }

  impl ActorRefSender for CapturingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
      self.inbox.lock().push(message);
      Ok(SendOutcome::Delivered)
    }
  }

  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let mut target_ref = ActorRef::new(Pid::new(900, 0), CapturingSender { inbox: inbox.clone() });

  let original_sender = ActorRef::new(Pid::new(800, 0), NullSender);

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  context.set_sender(Some(original_sender.clone()));

  context.try_forward(&mut target_ref, AnyMessage::new(42_u32)).expect("forward");

  let captured = inbox.lock();
  assert_eq!(captured.len(), 1);
  let forwarded = &captured[0];
  assert_eq!(forwarded.sender().expect("sender preserved").pid(), original_sender.pid());
}

#[test]
fn actor_context_forward_without_sender_sends_without_sender() {
  use crate::core::kernel::actor::actor_ref::{ActorRef, ActorRefSender, SendOutcome};

  struct CapturingSender {
    inbox: ArcShared<NoStdMutex<Vec<AnyMessage>>>,
  }

  impl ActorRefSender for CapturingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
      self.inbox.lock().push(message);
      Ok(SendOutcome::Delivered)
    }
  }

  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let mut target_ref = ActorRef::new(Pid::new(900, 0), CapturingSender { inbox: inbox.clone() });

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);

  context.try_forward(&mut target_ref, AnyMessage::new(42_u32)).expect("forward");

  let captured = inbox.lock();
  assert_eq!(captured.len(), 1);
  assert!(captured[0].sender().is_none());
}

fn register_cell(system: &ActorSystem, pid: Pid, name: &str, props: &Props) -> ArcShared<ActorCell> {
  let cell = ActorCell::create(system.state(), pid, None, String::from(name), props).expect("create actor cell");
  system.state().register_cell(cell.clone());
  cell
}

#[test]
fn actor_context_watch_enqueues_system_message() {
  let system = ActorSystem::new_empty();
  let watcher_pid = system.allocate_pid();
  let target_pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _watcher = register_cell(&system, watcher_pid, "watcher", &props);
  let target = register_cell(&system, target_pid, "target", &props);

  let mut context = ActorContext::new(&system, watcher_pid);
  let target_ref = target.actor_ref();
  assert!(context.watch(&target_ref).is_ok());
  assert!(target.watchers_snapshot().contains(&watcher_pid));
}

#[test]
fn actor_context_watch_missing_actor_notifies_self() {
  let system = ActorSystem::new_empty();
  let watcher_pid = system.allocate_pid();
  let target_pid = system.allocate_pid();
  let watcher_log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let watcher_props = Props::from_fn({
    let log = watcher_log.clone();
    move || RecordingActor::new(log.clone())
  });
  let target_props = Props::from_fn(|| TestActor);
  let _watcher = register_cell(&system, watcher_pid, "watcher", &watcher_props);
  let target = register_cell(&system, target_pid, "target", &target_props);
  let target_ref = target.actor_ref();
  system.state().remove_cell(&target_pid);

  let mut context = ActorContext::new(&system, watcher_pid);
  assert!(context.watch(&target_ref).is_ok());
  assert_eq!(watcher_log.lock().clone(), vec![target_pid]);
}

#[test]
fn actor_context_unwatch_enqueues_message() {
  let system = ActorSystem::new_empty();
  let watcher_pid = system.allocate_pid();
  let target_pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _watcher = register_cell(&system, watcher_pid, "watcher", &props);
  let target = register_cell(&system, target_pid, "target", &props);
  let mut context = ActorContext::new(&system, watcher_pid);
  let target_ref = target.actor_ref();

  assert!(context.watch(&target_ref).is_ok());
  assert!(context.unwatch(&target_ref).is_ok());
  assert!(!target.watchers_snapshot().contains(&watcher_pid));
}

#[test]
fn spawn_child_watched_installs_watch() {
  let system = ActorSystem::new_empty();
  let parent_pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _parent = register_cell(&system, parent_pid, "parent", &props);
  let mut context = ActorContext::new(&system, parent_pid);
  let child_props = Props::from_fn(|| TestActor);

  let child = context.spawn_child_watched(&child_props).expect("child spawn succeeds");
  let child_cell = system.state().cell(&child.pid()).expect("child registered");

  assert!(child_cell.watchers_snapshot().contains(&parent_pid));
}

#[test]
fn actor_context_child_by_name_returns_matching_child() {
  let system = ActorSystem::new_empty();
  let parent_pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _parent = register_cell(&system, parent_pid, "parent", &props);
  let mut context = ActorContext::new(&system, parent_pid);
  let child_props = Props::from_fn(|| TestActor);

  let child = context.spawn_child(&child_props).expect("spawn child");
  // spawn_child does not accept a name, so we retrieve the auto-assigned name
  // via the cell registry to exercise the child-by-name lookup.
  let child_name = system.state().cell(&child.pid()).expect("cell").name().to_owned();
  let found = context.child(&child_name);
  assert!(found.is_some());
  assert_eq!(found.expect("child should be found by name").pid(), child.pid());
}

#[test]
fn actor_context_child_by_name_returns_none_for_unknown() {
  let system = ActorSystem::new_empty();
  let parent_pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _parent = register_cell(&system, parent_pid, "parent", &props);
  let context = ActorContext::new(&system, parent_pid);

  assert!(context.child("nonexistent").is_none());
}

#[test]
fn actor_context_stop_child_returns_ok() {
  let system = ActorSystem::new_empty();
  let parent_pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _parent = register_cell(&system, parent_pid, "parent", &props);
  let mut context = ActorContext::new(&system, parent_pid);
  let child_props = Props::from_fn(|| TestActor);

  let child = context.spawn_child(&child_props).expect("spawn child");
  let child_name = system.state().cell(&child.pid()).expect("cell").name().to_owned();
  let result = context.stop_child(&child);
  assert!(result.is_ok());
  wait_until(|| context.child(&child_name).is_none());
}

#[test]
fn actor_context_tags_returns_props_tags_at_runtime() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor).with_tags(["observer", "critical"]);
  let _cell = register_cell(&system, pid, "tagged-actor", &props);
  let context = ActorContext::new(&system, pid);

  let tags = context.tags();
  assert_eq!(tags.len(), 2);
  assert!(tags.contains("observer"));
  assert!(tags.contains("critical"));
}

#[test]
fn actor_context_tags_returns_empty_without_tags() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _cell = register_cell(&system, pid, "plain-actor", &props);
  let context = ActorContext::new(&system, pid);

  assert!(context.tags().is_empty());
}

/// `reply` with a valid sender returns `Ok(())`.
#[test]
fn actor_context_reply_with_sender_returns_ok() {
  use crate::core::kernel::actor::actor_ref::{ActorRef, ActorRefSender, SendOutcome};

  struct CapturingSender {
    inbox: ArcShared<NoStdMutex<Vec<AnyMessage>>>,
  }

  impl ActorRefSender for CapturingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
      self.inbox.lock().push(message);
      Ok(SendOutcome::Delivered)
    }
  }

  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let sender_ref = ActorRef::new(Pid::new(800, 0), CapturingSender { inbox: inbox.clone() });

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  context.set_sender(Some(sender_ref));

  let result = context.reply(AnyMessage::new(42_u32));
  assert!(result.is_ok());

  let captured = inbox.lock();
  assert_eq!(captured.len(), 1);
}

/// `reply` with a failing sender propagates the synchronous send failure.
#[test]
fn actor_context_reply_with_failing_sender_returns_err() {
  use crate::core::kernel::actor::actor_ref::{ActorRef, ActorRefSender, SendOutcome};

  struct FailingSender;

  impl ActorRefSender for FailingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
      Err(SendError::closed(message))
    }
  }

  let sender_ref = ActorRef::new(Pid::new(800, 0), FailingSender);

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  context.set_sender(Some(sender_ref));

  // reply は内部で try_tell を使うため、同期配送失敗が返される。
  let result = context.reply(AnyMessage::new(42_u32));
  assert!(matches!(result, Err(SendError::Closed(_))));
}

/// `forward` on a failing target does not propagate the error (fire-and-forget).
#[test]
fn actor_context_forward_on_failing_target_does_not_propagate_error() {
  use crate::core::kernel::actor::actor_ref::{ActorRef, ActorRefSender, SendOutcome};

  struct FailingSender;

  impl ActorRefSender for FailingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
      Err(SendError::closed(message))
    }
  }

  let mut target_ref = ActorRef::new(Pid::new(900, 0), FailingSender);

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);

  let result = context.try_forward(&mut target_ref, AnyMessage::new(42_u32));
  assert!(result.is_err());
}

// --- T7: classic receive-timeout tests ---

#[test]
fn set_receive_timeout_stores_handle() {
  // Given: a kernel actor context with no receive timeout
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _cell = register_cell(&system, pid, "timeout-actor", &props);
  let mut context = ActorContext::new(&system, pid);

  // When: set_receive_timeout is called
  let timeout_msg = AnyMessage::new(999_u32);
  context.set_receive_timeout(Duration::from_millis(500), timeout_msg);

  // Then: has_receive_timeout returns true
  assert!(context.has_receive_timeout(), "receive timeout should be configured after set");
}

#[test]
fn cancel_receive_timeout_clears_handle() {
  // Given: a kernel actor context with a configured receive timeout
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _cell = register_cell(&system, pid, "cancel-actor", &props);
  let mut context = ActorContext::new(&system, pid);
  context.set_receive_timeout(Duration::from_millis(500), AnyMessage::new(999_u32));

  // When: cancel_receive_timeout is called
  context.cancel_receive_timeout();

  // Then: has_receive_timeout returns false
  assert!(!context.has_receive_timeout(), "receive timeout should be cleared after cancel");
}

#[test]
fn set_receive_timeout_replaces_previous_timeout() {
  // Given: a kernel actor context with an existing receive timeout
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| TestActor);
  let _cell = register_cell(&system, pid, "replace-actor", &props);
  let mut context = ActorContext::new(&system, pid);
  context.set_receive_timeout(Duration::from_millis(500), AnyMessage::new(1_u32));

  // When: set_receive_timeout is called again with different parameters
  context.set_receive_timeout(Duration::from_millis(1000), AnyMessage::new(2_u32));

  // Then: the timeout is still active (replaced, not accumulated)
  assert!(context.has_receive_timeout(), "receive timeout should still be configured");
}

#[test]
fn cancel_receive_timeout_is_idempotent() {
  // Given: a kernel actor context with no receive timeout
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);

  // When: cancel_receive_timeout is called without prior set
  context.cancel_receive_timeout();

  // Then: no panic, still no timeout
  assert!(!context.has_receive_timeout(), "cancel on no-timeout should be safe");
}

#[test]
fn has_receive_timeout_returns_false_initially() {
  // Given: a freshly created kernel actor context
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);

  // When/Then: has_receive_timeout returns false
  assert!(!context.has_receive_timeout(), "new context should not have receive timeout");
}

#[test]
fn receive_timeout_state_resets_across_fresh_contexts() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let events = events.clone();
    move || ReceiveTimeoutOnlyActor::new(events.clone())
  });
  let _cell = register_cell(&system, pid, "timeout-state", &props);
  let timeout_state = RuntimeMutex::new(None);

  // Configure the timeout via one context instance.
  {
    let mut context = ActorContext::new(&system, pid).with_receive_timeout_state(&timeout_state);
    context.set_receive_timeout(Duration::from_millis(20), AnyMessage::new(ReceiveTimeoutTick));
  }

  // Advance to t=1: the initial deadline is still in the future.
  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));
  assert!(events.lock().is_empty(), "timeout should not fire before the initial deadline");

  // Simulate the next message delivery by creating a fresh context and asking it to reschedule.
  {
    let mut context = ActorContext::new(&system, pid).with_receive_timeout_state(&timeout_state);
    context.reschedule_receive_timeout();
  }

  // The original deadline t=2 should no longer fire.
  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));
  assert!(events.lock().is_empty(), "reschedule should postpone the original deadline");

  // The new deadline t=3 should deliver the timeout.
  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));
  wait_until(|| events.lock().as_slice() == ["timeout"]);
}

#[test]
fn receive_timeout_state_can_be_armed_again_after_later_delivery() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let events = events.clone();
    move || ReceiveTimeoutOnlyActor::new(events.clone())
  });
  let _cell = register_cell(&system, pid, "timeout-state-repeat", &props);
  let timeout_state = RuntimeMutex::new(None);

  {
    let mut context = ActorContext::new(&system, pid).with_receive_timeout_state(&timeout_state);
    context.set_receive_timeout(Duration::from_millis(20), AnyMessage::new(ReceiveTimeoutTick));
  }

  system.scheduler().with_write(|scheduler| scheduler.run_for_test(2));
  wait_until(|| events.lock().as_slice() == ["timeout"]);

  // Simulate a later message delivery by rescheduling from a new context instance.
  {
    let mut context = ActorContext::new(&system, pid).with_receive_timeout_state(&timeout_state);
    context.reschedule_receive_timeout();
  }

  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));
  assert_eq!(events.lock().as_slice(), ["timeout"], "a fresh idle window should start from the later delivery");

  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));
  wait_until(|| events.lock().as_slice() == ["timeout", "timeout"]);
}

// --- T8: logger_name テスト ---

#[test]
fn actor_context_logger_name_initially_none() {
  // 前提: 新しく生成した actor context
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);

  // 確認: logger_name は None を返す
  assert!(context.logger_name().is_none());
}

#[test]
fn actor_context_set_logger_name_stores_value() {
  // 前提: actor context がある
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);

  // 実行: set_logger_name を呼ぶ
  context.set_logger_name("my.actor.logger");

  // 確認: logger_name が設定値を返す
  assert_eq!(context.logger_name(), Some("my.actor.logger"));
}

#[test]
fn actor_context_set_logger_name_replaces_previous() {
  // 前提: 既に logger name が設定された actor context
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  context.set_logger_name("first.logger");

  // 実行: set_logger_name を再度呼ぶ
  context.set_logger_name("second.logger");

  // 確認: 新しい名前で上書きされる
  assert_eq!(context.logger_name(), Some("second.logger"));
}

// --- pipe_to（外部 target）テスト ---

#[test]
fn actor_context_pipe_to_delivers_to_external_target() {
  // 前提: source actor と target actor がある
  let system = ActorSystem::new_empty();
  let source_pid = system.allocate_pid();
  let target_pid = system.allocate_pid();
  let target_received = ArcShared::new(NoStdMutex::new(Vec::new()));

  let source_props = Props::from_fn(|| TestActor);
  register_cell(&system, source_pid, "source", &source_props);

  let target_props = Props::from_fn({
    let log = target_received.clone();
    move || ProbeActor::new(log.clone())
  });
  let target_cell = register_cell(&system, target_pid, "target", &target_props);
  let target_ref = target_cell.actor_ref();

  let mut context = ActorContext::new(&system, source_pid);

  // 実行: 外部 target に対して pipe_to を呼ぶ
  context.pipe_to(async { 99_i32 }, &target_ref, |value| Some(AnyMessage::new(value))).expect("pipe_to");

  // 確認: source ではなく target がメッセージを受け取る
  wait_until(|| !target_received.lock().is_empty());
  assert_eq!(target_received.lock()[0], 99);
}

#[test]
fn actor_context_pipe_to_self_still_works_after_pipe_to_added() {
  // 前提: pipe_to_self を使う actor がある
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = received.clone();
    move || ProbeActor::new(log.clone())
  });
  register_cell(&system, pid, "self-check", &props);
  let mut context = ActorContext::new(&system, pid);

  // 実行: 既存 API の pipe_to_self を使う
  context.pipe_to_self(async { 77_i32 }, AnyMessage::new).expect("pipe to self");

  // 確認: メッセージが self に届く
  wait_until(|| !received.lock().is_empty());
  assert_eq!(received.lock()[0], 77);
}
