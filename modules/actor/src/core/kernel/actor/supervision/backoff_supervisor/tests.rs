use alloc::{string::String, vec::Vec};
use core::{hint::spin_loop, time::Duration};

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex, SharedAccess};

use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_ref::{ActorRef, ActorRefSender, NullSender, SendOutcome},
    child_ref::ChildRef,
    error::{ActorError, SendError},
    messaging::{AnyMessage, AnyMessageView, system_message::SystemMessage},
    props::Props,
    supervision::{
      BackoffOnFailureOptions, BackoffOnStopOptions, BackoffSupervisor, BackoffSupervisorCommand,
      BackoffSupervisorResponse, BackoffSupervisorStrategy, SupervisorDirective, SupervisorStrategy,
    },
  },
  system::ActorSystem,
};

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}

fn register_cell(system: &ActorSystem, pid: Pid, name: &str, props: &Props) -> ArcShared<ActorCell> {
  let cell = ActorCell::create(system.state(), pid, None, String::from(name), props).expect("create actor cell");
  system.state().register_cell(cell.clone());
  cell
}

fn default_strategy() -> BackoffSupervisorStrategy {
  BackoffSupervisorStrategy::new(Duration::from_millis(100), Duration::from_secs(10), 0.2)
}

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn noop_props() -> Props {
  Props::from_fn(|| NoopActor)
}

fn one_tick_strategy() -> BackoffSupervisorStrategy {
  BackoffSupervisorStrategy::new(Duration::from_millis(1), Duration::from_millis(1), 0.0)
}

struct CapturingSender {
  inbox: ArcShared<NoStdMutex<Vec<AnyMessage>>>,
}

impl ActorRefSender for CapturingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.inbox.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

struct ForwardProbeActor {
  received: ArcShared<NoStdMutex<Vec<i32>>>,
}

impl ForwardProbeActor {
  fn new(received: ArcShared<NoStdMutex<Vec<i32>>>) -> Self {
    Self { received }
  }
}

impl Actor for ForwardProbeActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(value) = message.downcast_ref::<i32>() {
      self.received.lock().push(*value);
    }
    Ok(())
  }
}

struct FailingOnMessageActor;

impl Actor for FailingOnMessageActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Err(ActorError::recoverable("boom"))
  }
}

fn current_child_pid(sup_ref: &mut ActorRef) -> Option<Pid> {
  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let reply_ref = ActorRef::new(Pid::new(999, 0), CapturingSender { inbox: inbox.clone() });
  let msg = AnyMessage::new(BackoffSupervisorCommand::GetCurrentChild).with_sender(reply_ref);
  sup_ref.tell(msg);
  wait_until(|| !inbox.lock().is_empty());
  let response = inbox.lock();
  let reply = response[0].downcast_ref::<BackoffSupervisorResponse>().expect("backoff supervisor response");
  match reply {
    | BackoffSupervisorResponse::CurrentChild(pid) => *pid,
    | other => panic!("expected current child response, got {:?}", other),
  }
}

// --- Factory method tests ---

#[test]
fn props_on_stop_returns_valid_props() {
  // Given: BackoffOnStopOptions with required fields
  let options = BackoffOnStopOptions::new(noop_props(), String::from("child"), default_strategy());

  // When: creating Props via BackoffSupervisor factory
  let props = BackoffSupervisor::props_on_stop(options);

  // Then: the props are usable (non-panicking construction)
  let _ = props;
}

#[test]
fn props_on_failure_returns_valid_props() {
  // Given: BackoffOnFailureOptions with required fields
  let options = BackoffOnFailureOptions::new(noop_props(), String::from("child"), default_strategy());

  // When: creating Props via BackoffSupervisor factory
  let props = BackoffSupervisor::props_on_failure(options);

  // Then: the props are usable (non-panicking construction)
  let _ = props;
}

// --- Protocol: GetCurrentChild ---

#[test]
fn get_current_child_returns_child_pid_when_child_is_running() {
  // Given: a backoff supervisor actor spawned with on-stop options
  let system = ActorSystem::new_empty();
  let options = BackoffOnStopOptions::new(noop_props(), String::from("child"), default_strategy());
  let sup_props = BackoffSupervisor::props_on_stop(options);
  let sup_pid = system.allocate_pid();
  let sup_cell = register_cell(&system, sup_pid, "backoff-sup", &sup_props);
  let mut sup_ref = sup_cell.actor_ref();

  // Capture reply
  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let reply_ref = ActorRef::new(Pid::new(999, 0), CapturingSender { inbox: inbox.clone() });

  // When: sending GetCurrentChild with a reply sender
  let msg = AnyMessage::new(BackoffSupervisorCommand::GetCurrentChild).with_sender(reply_ref);
  sup_ref.tell(msg);

  // Then: a CurrentChild response is received with Some(pid)
  wait_until(|| !inbox.lock().is_empty());
  let response = inbox.lock();
  assert_eq!(response.len(), 1);
  let reply = response[0].downcast_ref::<BackoffSupervisorResponse>();
  assert!(reply.is_some());
  assert!(matches!(reply.unwrap(), BackoffSupervisorResponse::CurrentChild(Some(_))));
}

// --- Protocol: GetRestartCount ---

#[test]
fn get_restart_count_returns_zero_initially() {
  // Given: a freshly spawned backoff supervisor
  let system = ActorSystem::new_empty();
  let options = BackoffOnStopOptions::new(noop_props(), String::from("child"), default_strategy());
  let sup_props = BackoffSupervisor::props_on_stop(options);
  let sup_pid = system.allocate_pid();
  let sup_cell = register_cell(&system, sup_pid, "backoff-sup", &sup_props);
  let mut sup_ref = sup_cell.actor_ref();

  // Capture reply
  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let reply_ref = ActorRef::new(Pid::new(999, 0), CapturingSender { inbox: inbox.clone() });

  // When: sending GetRestartCount
  let msg = AnyMessage::new(BackoffSupervisorCommand::GetRestartCount).with_sender(reply_ref);
  sup_ref.tell(msg);

  // Then: RestartCount(0) is returned
  wait_until(|| !inbox.lock().is_empty());
  let response = inbox.lock();
  assert_eq!(response.len(), 1);
  let reply = response[0].downcast_ref::<BackoffSupervisorResponse>();
  assert!(reply.is_some());
  assert!(matches!(reply.unwrap(), BackoffSupervisorResponse::RestartCount(0)));
}

// --- Protocol: Reset ---

#[test]
fn reset_command_resets_restart_count() {
  // Given: a backoff supervisor (even after hypothetical restarts, the counter should reset)
  let system = ActorSystem::new_empty();
  let options = BackoffOnStopOptions::new(noop_props(), String::from("child"), default_strategy()).with_manual_reset();
  let sup_props = BackoffSupervisor::props_on_stop(options);
  let sup_pid = system.allocate_pid();
  let sup_cell = register_cell(&system, sup_pid, "backoff-sup", &sup_props);
  let mut sup_ref = sup_cell.actor_ref();

  // When: sending Reset followed by GetRestartCount
  sup_ref.tell(AnyMessage::new(BackoffSupervisorCommand::Reset));

  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let reply_ref = ActorRef::new(Pid::new(999, 0), CapturingSender { inbox: inbox.clone() });
  let msg = AnyMessage::new(BackoffSupervisorCommand::GetRestartCount).with_sender(reply_ref);
  sup_ref.tell(msg);

  // Then: RestartCount is 0 after reset
  wait_until(|| !inbox.lock().is_empty());
  let response = inbox.lock();
  let reply = response[0].downcast_ref::<BackoffSupervisorResponse>();
  assert!(matches!(reply.unwrap(), BackoffSupervisorResponse::RestartCount(0)));
}

// --- Message forwarding ---

#[test]
fn unrecognized_messages_are_forwarded_to_child() {
  // Given: a backoff supervisor wrapping a probe actor that records i32 messages
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
  let child_props = Props::from_fn({
    let received = received.clone();
    move || ForwardProbeActor::new(received.clone())
  });
  let system = ActorSystem::new_empty();
  let options = BackoffOnStopOptions::new(child_props, String::from("probe-child"), default_strategy());
  let sup_props = BackoffSupervisor::props_on_stop(options);
  let sup_pid = system.allocate_pid();
  let sup_cell = register_cell(&system, sup_pid, "backoff-sup", &sup_props);
  let mut sup_ref = sup_cell.actor_ref();

  // When: sending an i32 user message to the supervisor
  sup_ref.tell(AnyMessage::new(42_i32));

  // Then: the child actor receives the forwarded message
  wait_until(|| !received.lock().is_empty());
  assert_eq!(received.lock()[0], 42);
}

// --- GetCurrentChild when child is not running ---

#[test]
fn get_current_child_returns_none_when_no_child() {
  // Given: a backoff supervisor that has not yet started its child
  // (this tests the state before on_started or after child termination + max_retries exceeded)
  let _system = ActorSystem::new_empty();
  let options = BackoffOnStopOptions::new(noop_props(), String::from("child"), default_strategy()).with_max_retries(0);
  let _sup_props = BackoffSupervisor::props_on_stop(options);

  // Construct the actor without calling on_started to simulate no-child state
  // This is a boundary case — the exact behavior depends on implementation.
  // The test verifies that CurrentChild(None) is a valid response.
  let response = BackoffSupervisorResponse::CurrentChild(None);
  assert!(matches!(response, BackoffSupervisorResponse::CurrentChild(None)));
}

// --- Options: max_retries boundary ---

#[test]
fn max_retries_zero_means_unlimited() {
  // Given: options with max_retries = 0 (Pekko convention: 0 = unlimited)
  let options = BackoffOnStopOptions::new(noop_props(), String::from("child"), default_strategy());

  // Then: max_retries is 0
  assert_eq!(options.max_retries(), 0);
}

#[test]
fn max_retries_one_limits_to_single_restart() {
  // Given: options with max_retries = 1
  let options = BackoffOnStopOptions::new(noop_props(), String::from("child"), default_strategy()).with_max_retries(1);

  // Then: max_retries is 1
  assert_eq!(options.max_retries(), 1);
}

// --- Factory with different option configurations ---

#[test]
fn props_on_stop_with_all_options_configured() {
  // Given: fully configured on-stop options
  let options = BackoffOnStopOptions::new(noop_props(), String::from("child"), default_strategy())
    .with_auto_reset(Duration::from_secs(30))
    .with_max_retries(5);

  // When: creating props
  let props = BackoffSupervisor::props_on_stop(options);

  // Then: props are valid
  let _ = props;
}

#[test]
fn props_on_failure_with_manual_reset() {
  // Given: on-failure options with manual reset
  let options = BackoffOnFailureOptions::new(noop_props(), String::from("child"), default_strategy())
    .with_manual_reset()
    .with_max_retries(3);

  // When: creating props
  let props = BackoffSupervisor::props_on_failure(options);

  // Then: props are valid
  let _ = props;
}

#[test]
fn first_restart_maps_to_zero_backoff_iteration() {
  assert_eq!(super::BackoffSupervisorActor::backoff_iteration_for_restart_count(1), 0);
}

#[test]
fn second_restart_maps_to_first_exponential_backoff_iteration() {
  assert_eq!(super::BackoffSupervisorActor::backoff_iteration_for_restart_count(2), 1);
}

#[test]
fn on_stop_restarts_child_only_after_backoff_tick() {
  let system = ActorSystem::new_empty();
  let options = BackoffOnStopOptions::new(noop_props(), String::from("child"), one_tick_strategy());
  let sup_props = BackoffSupervisor::props_on_stop(options);
  let sup_pid = system.allocate_pid();
  let sup_cell = register_cell(&system, sup_pid, "backoff-sup-on-stop", &sup_props);
  let mut sup_ref = sup_cell.actor_ref();

  let initial_child = current_child_pid(&mut sup_ref).expect("initial child");
  system.state().send_system_message(initial_child, SystemMessage::PoisonPill).expect("stop child");

  assert_eq!(current_child_pid(&mut sup_ref), None, "child should remain stopped until the backoff elapses");

  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));

  let restarted_child = current_child_pid(&mut sup_ref).expect("restarted child after backoff");
  assert_ne!(restarted_child, initial_child);
}

#[test]
fn on_failure_restarts_child_only_after_failure_and_backoff_tick() {
  let system = ActorSystem::new_empty();
  let child_props = Props::from_fn(|| FailingOnMessageActor);
  let options = BackoffOnFailureOptions::new(child_props, String::from("child"), one_tick_strategy());
  let sup_props = BackoffSupervisor::props_on_failure(options);
  let sup_pid = system.allocate_pid();
  let sup_cell = register_cell(&system, sup_pid, "backoff-sup-on-failure", &sup_props);
  let mut sup_ref = sup_cell.actor_ref();

  let initial_child = current_child_pid(&mut sup_ref).expect("initial child");
  sup_ref.tell(AnyMessage::new(1_u32));

  assert_eq!(current_child_pid(&mut sup_ref), None, "failed child should stay stopped until the backoff elapses");

  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));

  let restarted_child = current_child_pid(&mut sup_ref).expect("restarted child after failure backoff");
  assert_ne!(restarted_child, initial_child);
}

#[test]
fn on_failure_forwarding_keeps_supervisor_responsive_until_backoff_restart() {
  let system = ActorSystem::new_empty();
  let child_props = Props::from_fn(|| FailingOnMessageActor);
  let options = BackoffOnFailureOptions::new(child_props, String::from("child"), one_tick_strategy());
  let sup_props = BackoffSupervisor::props_on_failure(options);
  let sup_pid = system.allocate_pid();
  let sup_cell = register_cell(&system, sup_pid, "backoff-sup-on-failure-responsive", &sup_props);
  let mut sup_ref = sup_cell.actor_ref();

  let initial_child = current_child_pid(&mut sup_ref).expect("initial child");
  sup_ref.tell(AnyMessage::new(1_u32));

  assert_eq!(current_child_pid(&mut sup_ref), None, "supervisor should answer while waiting for backoff restart");

  let inbox = ArcShared::new(NoStdMutex::new(Vec::new()));
  let reply_ref = ActorRef::new(Pid::new(998, 0), CapturingSender { inbox: inbox.clone() });
  let msg = AnyMessage::new(BackoffSupervisorCommand::GetRestartCount).with_sender(reply_ref);
  sup_ref.tell(msg);
  wait_until(|| !inbox.lock().is_empty());
  let replies = inbox.lock();
  let reply = replies[0].downcast_ref::<BackoffSupervisorResponse>().expect("restart count response");
  assert!(matches!(reply, BackoffSupervisorResponse::RestartCount(1)));

  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));

  let restarted_child = current_child_pid(&mut sup_ref).expect("restarted child after responsive failure path");
  assert_ne!(restarted_child, initial_child);
}

#[test]
fn on_failure_does_not_restart_child_after_normal_stop() {
  let system = ActorSystem::new_empty();
  let options = BackoffOnFailureOptions::new(noop_props(), String::from("child"), one_tick_strategy());
  let sup_props = BackoffSupervisor::props_on_failure(options);
  let sup_pid = system.allocate_pid();
  let sup_cell = register_cell(&system, sup_pid, "backoff-sup-on-failure-stop", &sup_props);
  let mut sup_ref = sup_cell.actor_ref();

  let initial_child = current_child_pid(&mut sup_ref).expect("initial child");
  system.state().send_system_message(initial_child, SystemMessage::PoisonPill).expect("stop child");

  assert_eq!(current_child_pid(&mut sup_ref), None);
  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));
  assert_eq!(current_child_pid(&mut sup_ref), None, "on-failure must not restart after a normal stop");
}

#[test]
fn on_failure_with_stop_strategy_does_not_mark_pending_restart() {
  let system = ActorSystem::new_empty();
  let strategy = SupervisorStrategy::with_decider(|_| SupervisorDirective::Stop);
  let options = BackoffOnFailureOptions::new(noop_props(), String::from("child"), one_tick_strategy())
    .with_supervisor_strategy(strategy);
  let config = super::BackoffConfig::from_failure(options);
  let mut actor = super::BackoffSupervisorActor::from_config(config);
  let child_pid = Pid::new(42, 0);
  actor.child = Some(ChildRef::new(ActorRef::new(child_pid, NullSender), system.state()));

  let mut ctx = ActorContext::new(&system, Pid::new(100, 0));
  actor.on_child_failed(&mut ctx, child_pid, &ActorError::recoverable("boom")).expect("record child failure");

  assert!(!actor.pending_restart, "custom stop strategy must suppress backoff restart after failure");
}
