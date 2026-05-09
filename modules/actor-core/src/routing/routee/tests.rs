use alloc::{format, string::ToString, vec};
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::Routee;
use crate::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_path::ActorPathParser,
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    error::{ActorError, SendError},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::{SchedulerConfig, tick_driver::tests::TestTickDriver},
    setup::ActorSystemConfig,
  },
  system::{
    remote::RemotingConfig,
    state::{SystemStateShared, system_state::SystemState},
  },
};

// ---------------------------------------------------------------------------
// Helper: CapturingSender
// ---------------------------------------------------------------------------

/// Sender that records the number of messages received.
struct CapturingSender {
  count: ArcShared<AtomicUsize>,
}

impl CapturingSender {
  fn new() -> (ArcShared<AtomicUsize>, Self) {
    let count = ArcShared::new(AtomicUsize::new(0));
    let sender = Self { count: count.clone() };
    (count, sender)
  }
}

impl ActorRefSender for CapturingSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.count.fetch_add(1, Ordering::Relaxed);
    Ok(SendOutcome::Delivered)
  }
}

/// Sender that always rejects messages with a closed error.
struct ClosedSender;

impl ActorRefSender for ClosedSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::closed(message))
  }
}

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn build_actor_ref_with_system() -> (ActorRef, SystemStateShared) {
  let scheduler = SchedulerConfig::default().with_runner_api_enabled(true);
  let remoting = RemotingConfig::default().with_canonical_host("10.0.0.1").with_canonical_port(2552);
  let config = ActorSystemConfig::default()
    .with_system_name("remote-sys")
    .with_scheduler_config(scheduler)
    .with_tick_driver(TestTickDriver::default())
    .with_remoting_config(remoting);
  let state = SystemStateShared::new(SystemState::build_from_owned_config(config).expect("state"));

  let props = Props::from_fn(|| NoopActor);
  let root_pid = state.allocate_pid();
  let root = ActorCell::create(state.clone(), root_pid, None, "root".to_string(), &props).expect("root cell");
  state.register_cell(root);

  let child_pid = state.allocate_pid();
  let child =
    ActorCell::create(state.clone(), child_pid, Some(root_pid), "worker".to_string(), &props).expect("child cell");
  state.register_cell(child.clone());

  (child.actor_ref(), state)
}

// ---------------------------------------------------------------------------
// Construction tests
// ---------------------------------------------------------------------------

#[test]
fn actorref_variant_is_constructible() {
  // 前提: 有効な ActorRef がある
  let (_, sender) = CapturingSender::new();
  let actor_ref = ActorRef::new_with_builtin_lock(Pid::new(1, 0), sender);

  // 実行: Routee::ActorRef で包む
  let routee = Routee::ActorRef(actor_ref);

  // 確認: ActorRef variant になる
  assert!(matches!(routee, Routee::ActorRef(_)));
}

#[test]
fn no_routee_variant_is_constructible() {
  // Given/When: constructing a NoRoutee
  let routee = Routee::NoRoutee;

  // Then: it should be the NoRoutee variant
  assert!(matches!(routee, Routee::NoRoutee));
}

#[test]
fn several_variant_is_constructible() {
  // Given: a vec of routees
  let routees = vec![Routee::NoRoutee, Routee::NoRoutee];

  // When: wrapping in Several
  let routee = Routee::Several(routees);

  // Then: it should be the Several variant
  assert!(matches!(routee, Routee::Several(_)));
}

// ---------------------------------------------------------------------------
// Equality tests
// ---------------------------------------------------------------------------

#[test]
fn variants_are_distinct() {
  // Given: different variant kinds
  let (_, sender) = CapturingSender::new();
  let actor_ref_routee = Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(1, 0), sender));
  let no_routee = Routee::NoRoutee;
  let several = Routee::Several(vec![Routee::NoRoutee]);

  // Then: they should not be equal to each other
  assert_ne!(actor_ref_routee, no_routee);
  assert_ne!(no_routee, several);
  assert_ne!(actor_ref_routee, several);
}

#[test]
fn partial_eq_actorref_compares_by_pid() {
  // Given: two ActorRef routees with the same pid
  let (_, sender1) = CapturingSender::new();
  let (_, sender2) = CapturingSender::new();
  let routee1 = Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(42, 0), sender1));
  let routee2 = Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(42, 0), sender2));

  // Then: they should be equal (ActorRef compares by pid)
  assert_eq!(routee1, routee2);
}

#[test]
fn partial_eq_actorref_delegates_to_actor_ref_equality() {
  let (_, sender1) = CapturingSender::new();
  let (_, sender2) = CapturingSender::new();
  let path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  let routee1 = Routee::ActorRef(ActorRef::with_canonical_path(Pid::new(42, 0), sender1, path.clone()));
  let routee2 = Routee::ActorRef(ActorRef::with_canonical_path(Pid::new(43, 0), sender2, path));

  assert_eq!(routee1, routee2);
}

#[test]
fn partial_eq_actorref_system_and_pid_only_same_pid_are_equal() {
  let (system_ref, _state) = build_actor_ref_with_system();
  let pid = system_ref.pid();
  let (_, sender) = CapturingSender::new();
  let pid_only_ref = ActorRef::new_with_builtin_lock(pid, sender);

  let routee1 = Routee::ActorRef(system_ref);
  let routee2 = Routee::ActorRef(pid_only_ref);

  assert_eq!(routee1, routee2);
}

#[test]
fn partial_eq_no_routee_is_equal() {
  // Given: two NoRoutee values
  let a = Routee::NoRoutee;
  let b = Routee::NoRoutee;

  // Then: they should be equal
  assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// Send tests
// ---------------------------------------------------------------------------

#[test]
fn send_on_actorref_delegates_to_try_tell() {
  // Given: an ActorRef routee backed by a capturing sender
  let (count, sender) = CapturingSender::new();
  let actor_ref = ActorRef::new_with_builtin_lock(Pid::new(1, 0), sender);
  let mut routee = Routee::ActorRef(actor_ref);

  // When: sending a message
  let result = routee.send(AnyMessage::new(42_u32));

  // Then: the message should have been forwarded
  assert!(result.is_ok());
  assert_eq!(count.load(Ordering::Relaxed), 1);
}

#[test]
fn send_on_no_routee_returns_ok() {
  // Given: a NoRoutee
  let mut routee = Routee::NoRoutee;

  // When: sending a message
  let result = routee.send(AnyMessage::new(42_u32));

  // Then: it should succeed silently
  assert!(result.is_ok());
}

#[test]
fn send_on_several_sends_to_all() {
  // Given: Several with 3 capturing senders
  let (count1, sender1) = CapturingSender::new();
  let (count2, sender2) = CapturingSender::new();
  let (count3, sender3) = CapturingSender::new();
  let mut routee = Routee::Several(vec![
    Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(1, 0), sender1)),
    Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(2, 0), sender2)),
    Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(3, 0), sender3)),
  ]);

  // When: sending a message
  let result = routee.send(AnyMessage::new(42_u32));

  // Then: all three should have received the message
  assert!(result.is_ok());
  assert_eq!(count1.load(Ordering::Relaxed), 1);
  assert_eq!(count2.load(Ordering::Relaxed), 1);
  assert_eq!(count3.load(Ordering::Relaxed), 1);
}

#[test]
fn send_on_several_keeps_delivering_after_first_error() {
  // Given: Several with [ok, closed, ok]
  let (count_ok, sender_ok) = CapturingSender::new();
  let (count_ok2, sender_ok2) = CapturingSender::new();
  let mut routee = Routee::Several(vec![
    Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(1, 0), sender_ok)),
    Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(2, 0), ClosedSender)),
    Routee::ActorRef(ActorRef::new_with_builtin_lock(Pid::new(3, 0), sender_ok2)),
  ]);

  // When: sending a message
  let result = routee.send(AnyMessage::new(42_u32));

  // Then: 最初のエラーは返すが、後続 routee への配信は継続する
  assert!(result.is_err());
  assert_eq!(count_ok.load(Ordering::Relaxed), 1);
  assert_eq!(count_ok2.load(Ordering::Relaxed), 1);
}

// ---------------------------------------------------------------------------
// Clone / Debug tests
// ---------------------------------------------------------------------------

#[test]
fn clone_preserves_variant() {
  // Given: a NoRoutee (simplest cloneable variant)
  let routee = Routee::NoRoutee;

  // When: cloning
  let cloned = routee.clone();

  // Then: the clone should be equal to the original
  assert_eq!(routee, cloned);
}

#[test]
fn debug_format_is_non_empty() {
  // Given: a NoRoutee
  let routee = Routee::NoRoutee;

  // When: formatting with Debug
  let debug_str = format!("{:?}", routee);

  // Then: the output should be non-empty
  assert!(!debug_str.is_empty());
}
