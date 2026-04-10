use std::{
  env,
  process::{Command, Stdio},
  sync::{
    Arc, Barrier,
    atomic::{AtomicUsize, Ordering},
    mpsc::{self, Sender},
  },
  thread,
  time::{Duration, Instant},
};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    error::{ActorError, SendError},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  event::{
    logging::{LogEvent, LogLevel},
    stream::{EventStreamEvent, EventStreamShared, EventStreamSubscriber, subscriber_handle_with_lock_provider},
  },
  system::ActorSystem,
};
use fraktor_utils_adaptor_std_rs::std::sync::DebugSpinSyncMutex;
use fraktor_utils_core_rs::core::sync::SharedLock;

use super::{DebugActorLockProvider, StdActorLockProvider};

struct SelfLoopActor {
  delivered:          Arc<AtomicUsize>,
  forwards_remaining: Arc<AtomicUsize>,
}

impl Actor for SelfLoopActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    self.delivered.fetch_add(1, Ordering::SeqCst);
    let should_forward =
      self.forwards_remaining.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok();
    if should_forward {
      let mut target = ctx.self_ref();
      target.tell(AnyMessage::new(1_u32));
    }
    Ok(())
  }
}

fn build_debug_system() -> ActorSystem {
  ActorSystem::new_empty_with(|config| config.with_lock_provider(DebugActorLockProvider::new()))
}

fn build_std_system() -> ActorSystem {
  ActorSystem::new_empty_with(|config| config.with_lock_provider(StdActorLockProvider::new()))
}

fn build_default_system() -> ActorSystem {
  ActorSystem::new_empty()
}

fn build_self_loop_actor(system: &ActorSystem) -> (ActorRef, Arc<AtomicUsize>) {
  let state = system.state();
  let delivered = Arc::new(AtomicUsize::new(0));
  let forwards_remaining = Arc::new(AtomicUsize::new(1));
  let props = {
    let delivered = delivered.clone();
    let forwards_remaining = forwards_remaining.clone();
    Props::from_fn(move || SelfLoopActor {
      delivered:          delivered.clone(),
      forwards_remaining: forwards_remaining.clone(),
    })
  };
  let pid = state.allocate_pid();
  let cell = ActorCell::create(state.clone(), pid, None, "self-loop".into(), &props).expect("self-loop cell");
  state.register_cell(cell.clone());
  (cell.actor_ref(), delivered)
}

#[test]
fn debug_provider_allows_same_thread_reentrant_tell_after_sender_lock_release() {
  let system = build_debug_system();
  let (mut actor_ref, delivered) = build_self_loop_actor(&system);
  actor_ref.tell(AnyMessage::new(1_u32));
  assert_eq!(delivered.load(Ordering::SeqCst), 2, "debug provider should allow the nested self tell");
}

#[test]
fn default_fallback_and_system_scoped_override_remain_independent() {
  let default_system = build_default_system();
  let (mut default_actor_ref, default_delivered) = build_self_loop_actor(&default_system);
  default_actor_ref.tell(AnyMessage::new(1_u32));
  assert_eq!(default_delivered.load(Ordering::SeqCst), 2, "default provider should allow the nested self tell");

  let debug_system = build_debug_system();
  let (mut debug_actor_ref, debug_delivered) = build_self_loop_actor(&debug_system);
  debug_actor_ref.tell(AnyMessage::new(1_u32));
  assert_eq!(debug_delivered.load(Ordering::SeqCst), 2, "debug override should preserve the nested self tell contract");
}

#[test]
fn std_provider_builds_a_system_and_delivers_messages() {
  let system = build_std_system();
  let (mut actor_ref, delivered) = build_self_loop_actor(&system);
  actor_ref.tell(AnyMessage::new(1_u32));
  assert_eq!(delivered.load(Ordering::SeqCst), 2, "std provider should build a working system");
}

struct DeferredScheduleSender {
  send_count:             Arc<AtomicUsize>,
  first_schedule_entered: Sender<()>,
  first_schedule_release: Arc<Barrier>,
}

impl ActorRefSender for DeferredScheduleSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    let send_index = self.send_count.fetch_add(1, Ordering::SeqCst);
    if send_index == 0 {
      let first_schedule_entered = self.first_schedule_entered.clone();
      let first_schedule_release = self.first_schedule_release.clone();
      return Ok(SendOutcome::Schedule(Box::new(move || {
        first_schedule_entered.send(()).expect("first schedule should notify the test");
        first_schedule_release.wait();
      })));
    }

    Ok(SendOutcome::Delivered)
  }
}

struct ReentrantPublishSubscriber {
  stream:      EventStreamShared,
  delivered:   Arc<AtomicUsize>,
  republished: bool,
}

impl ReentrantPublishSubscriber {
  fn new(stream: EventStreamShared, delivered: Arc<AtomicUsize>) -> Self {
    Self { stream, delivered, republished: false }
  }
}

impl EventStreamSubscriber for ReentrantPublishSubscriber {
  fn on_event(&mut self, _event: &EventStreamEvent) {
    self.delivered.fetch_add(1, Ordering::SeqCst);
    if !self.republished {
      self.republished = true;
      self.stream.publish(&EventStreamEvent::Log(LogEvent::new(
        LogLevel::Info,
        "nested event".into(),
        Duration::from_millis(2),
        None,
        None,
      )));
    }
  }
}

#[test]
fn debug_driver_allows_parallel_send_after_releasing_sender_lock() {
  let (first_schedule_entered_tx, first_schedule_entered_rx) = mpsc::channel();
  let first_schedule_release = Arc::new(Barrier::new(2));
  let sender = ActorRefSenderShared::from_shared_lock(SharedLock::new_with_driver::<
    DebugSpinSyncMutex<Box<dyn ActorRefSender>>,
  >(Box::new(DeferredScheduleSender {
    send_count:             Arc::new(AtomicUsize::new(0)),
    first_schedule_entered: first_schedule_entered_tx,
    first_schedule_release: first_schedule_release.clone(),
  })));

  let mut first_sender = sender.clone();
  let first_handle = thread::spawn(move || first_sender.send(AnyMessage::new(1_u8)));

  first_schedule_entered_rx.recv().expect("first send should enter deferred schedule");

  let mut second_sender = sender.clone();
  let second_handle = thread::spawn(move || second_sender.send(AnyMessage::new(2_u8)));
  let second_join = second_handle.join();

  first_schedule_release.wait();

  let first_result = first_handle.join().expect("first send thread should not panic");
  let second_result = second_join.expect("second send thread should not panic");

  assert!(first_result.is_ok(), "first send should succeed");
  assert!(second_result.is_ok(), "second send should succeed without false nested-send detection");
}

#[test]
fn debug_provider_should_detect_reentrant_event_stream_subscriber_locking() {
  let exe = env::current_exe().expect("current test binary");
  let mut child = Command::new(exe)
    .arg("--exact")
    .arg("std::system::lock_provider::tests::debug_provider_reentrant_event_stream_publish_worker")
    .env("FRAKTOR_REENTRANT_EVENT_STREAM_CHILD", "1")
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .expect("spawn worker test");
  let deadline = Instant::now() + Duration::from_millis(200);

  loop {
    if let Some(status) = child.try_wait().expect("poll worker status") {
      assert!(
        !status.success(),
        "after section 3 the worker should fail fast with debug reentry detection instead of completing successfully"
      );
      return;
    }
    if Instant::now() >= deadline {
      child.kill().expect("terminate hung worker");
      panic!(
        "debug provider path still hangs on built-in event stream subscriber locking; section 3 should make it fail fast"
      );
    }
    thread::yield_now();
  }
}

#[test]
fn debug_provider_reentrant_event_stream_publish_worker() {
  if env::var_os("FRAKTOR_REENTRANT_EVENT_STREAM_CHILD").is_none() {
    return;
  }

  let system = build_debug_system();
  let stream = system.event_stream();
  let delivered = Arc::new(AtomicUsize::new(0));
  let subscriber = subscriber_handle_with_lock_provider(
    &system.state().lock_provider(),
    ReentrantPublishSubscriber::new(stream.clone(), delivered.clone()),
  );
  let _subscription = stream.subscribe(&subscriber);

  stream.publish(&EventStreamEvent::Log(LogEvent::new(
    LogLevel::Info,
    "outer event".into(),
    Duration::from_millis(1),
    None,
    None,
  )));

  assert_eq!(
    delivered.load(Ordering::SeqCst),
    1,
    "after section 3 the nested publish should panic before a second callback acquires the same subscriber lock"
  );
}
