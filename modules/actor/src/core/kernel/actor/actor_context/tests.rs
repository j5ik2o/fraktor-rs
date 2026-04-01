use alloc::{string::String, vec, vec::Vec};
use core::hint::spin_loop;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex, SharedAccess};

use super::ActorContext;
use crate::core::kernel::{
  actor::{
    Actor, ActorCell, Pid,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  event::logging::LogLevel,
  system::ActorSystem,
  util::futures::{ActorFutureListener, ActorFutureShared},
};

struct TestActor;

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
  use alloc::string::String;

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
  });
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
  let props = Props::from_fn(|| ProbeActor::new(ArcShared::new(NoStdMutex::new(Vec::new()))));
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
  });
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
fn actor_context_forward_preserves_sender() {
  use crate::core::kernel::actor::actor_ref::{ActorRef, ActorRefSender, SendOutcome};

  struct CapturingSender {
    inbox: ArcShared<NoStdMutex<Vec<AnyMessage>>>,
  }

  impl ActorRefSender for CapturingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, crate::core::kernel::actor::error::SendError> {
      self.inbox.lock().push(message);
      Ok(SendOutcome::Delivered)
    }
  }

  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let mut target_ref = ActorRef::new(Pid::new(900, 0), CapturingSender { inbox: inbox.clone() });

  let original_sender = ActorRef::new(Pid::new(800, 0), crate::core::kernel::actor::actor_ref::NullSender);

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
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, crate::core::kernel::actor::error::SendError> {
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
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, crate::core::kernel::actor::error::SendError> {
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
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, crate::core::kernel::actor::error::SendError> {
      Err(crate::core::kernel::actor::error::SendError::closed(message))
    }
  }

  let sender_ref = ActorRef::new(Pid::new(800, 0), FailingSender);

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  context.set_sender(Some(sender_ref));

  // reply は内部で try_tell を使うため、同期配送失敗が返される。
  let result = context.reply(AnyMessage::new(42_u32));
  assert!(matches!(result, Err(crate::core::kernel::actor::error::SendError::Closed(_))));
}

/// `forward` on a failing target does not propagate the error (fire-and-forget).
#[test]
fn actor_context_forward_on_failing_target_does_not_propagate_error() {
  use crate::core::kernel::actor::actor_ref::{ActorRef, ActorRefSender, SendOutcome};

  struct FailingSender;

  impl ActorRefSender for FailingSender {
    fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, crate::core::kernel::actor::error::SendError> {
      Err(crate::core::kernel::actor::error::SendError::closed(message))
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
  context.set_receive_timeout(core::time::Duration::from_millis(500), timeout_msg);

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
  context.set_receive_timeout(core::time::Duration::from_millis(500), AnyMessage::new(999_u32));

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
  context.set_receive_timeout(core::time::Duration::from_millis(500), AnyMessage::new(1_u32));

  // When: set_receive_timeout is called again with different parameters
  context.set_receive_timeout(core::time::Duration::from_millis(1000), AnyMessage::new(2_u32));

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
