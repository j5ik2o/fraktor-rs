use alloc::{string::String, vec::Vec};
use core::{hash::Hash, time::Duration};

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::{Fsm, FsmNamedTimer};
use crate::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    error::ActorError,
    fsm::{FsmStateTimeout, FsmTransition},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::SchedulerError,
  },
  system::ActorSystem,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ProbeState {
  Idle,
  Active,
}

struct ProbeActor;

impl Actor for ProbeActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

impl<State, Data> Fsm<State, Data>
where
  State: Clone + Eq + Hash + Send + Sync + 'static,
  Data: Clone + Send + Sync + 'static,
{
  /// Sets the named timer generation counter for wrap-around tests.
  pub(crate) const fn set_named_timer_generation_for_test(&mut self, generation: u64) {
    self.named_timer_generation = generation;
  }

  /// Returns the active named timer generation for tests.
  pub(crate) fn named_timer_generation_for_test(&self, name: &str) -> Option<u64> {
    self.named_timers.get(name).map(|timer| timer.generation())
  }
}

fn register_cell(system: &ActorSystem, pid: Pid, name: &str, props: &Props) -> ArcShared<ActorCell> {
  let cell = ActorCell::create(system.state(), pid, None, String::from(name), props).expect("create actor cell");
  system.state().register_cell(cell.clone());
  cell
}

fn build_context() -> (ActorSystem, ActorContext<'static>) {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| ProbeActor);
  let _cell = register_cell(&system, pid, "fsm-machine", &props);
  (system.clone(), ActorContext::new(&system, pid))
}

#[test]
fn scheduler_error_to_actor_error_preserves_scheduler_source_type() {
  let error = Fsm::<&'static str, ()>::scheduler_error_to_actor_error(&SchedulerError::Closed);

  assert!(matches!(error, ActorError::Recoverable(_)));
  assert!(error.is_source_type::<SchedulerError>());
  assert!(error.reason().as_str().contains("Closed"));
}

#[test]
fn initialize_without_start_with_returns_recoverable_error() {
  let (_system, context) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();

  let error = fsm.initialize(&context).expect_err("start_with is required");

  assert!(matches!(error, ActorError::Recoverable(_)));
}

#[test]
fn cancel_timer_with_missing_name_is_noop() {
  let (_system, context) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();

  assert!(fsm.cancel_timer(&context, "missing").is_ok());
}

#[test]
fn handle_without_registered_handler_is_noop() {
  let (_system, mut context) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 1);
  fsm.initialize(&context).expect("initialize");
  let message = AnyMessage::new(1_u32);
  let view = message.as_view();

  fsm.handle(&mut context, &view).expect("handle");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));
  assert_eq!(fsm.state_data(), Some(&1));
}

#[test]
fn when_unhandled_handles_missing_state_handler() {
  let (_system, mut context) = build_context();
  let calls = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let calls_for_handler = calls.clone();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 1);
  fsm.when_unhandled(move |_ctx, _message, state, data| {
    calls_for_handler.lock().push(state.clone());
    Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1))
  });
  fsm.initialize(&context).expect("initialize");
  let message = AnyMessage::new(1_u32);
  let view = message.as_view();

  fsm.handle(&mut context, &view).expect("handle");

  assert_eq!(calls.lock().as_slice(), &[ProbeState::Idle]);
  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.state_data(), Some(&2));
}

#[test]
fn reschedule_state_timeout_without_state_is_noop() {
  let (_system, context) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();

  assert!(fsm.reschedule_state_timeout(&context).is_ok());
}

#[test]
fn stale_timeout_without_current_state_is_ignored() {
  let (_system, mut context) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.initialized = true;
  fsm.data = Some(1);
  let timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, 1));
  let view = timeout.as_view();

  fsm.handle(&mut context, &view).expect("stale timeout");

  assert_eq!(fsm.state_data(), Some(&1));
}

#[test]
fn cancel_replaced_named_timer_logs_scheduler_error_when_actor_is_missing() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.named_timers.insert(String::from("tick"), FsmNamedTimer::new(1, false, String::from("timer-key")));

  fsm.cancel_replaced_named_timer(&context, "tick");

  assert!(!fsm.is_timer_active("tick"));
}

#[test]
fn cancel_all_named_timers_logs_scheduler_errors_when_actor_is_missing() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let context = ActorContext::new(&system, pid);
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.named_timers.insert(String::from("first"), FsmNamedTimer::new(1, false, String::from("first-key")));
  fsm.named_timers.insert(String::from("second"), FsmNamedTimer::new(2, true, String::from("second-key")));

  fsm.cancel_all_named_timers_best_effort(&context);

  assert!(fsm.named_timers.is_empty());
}

#[test]
fn start_named_timer_returns_typed_scheduler_error_when_schedule_fails() {
  let (_system, context) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();

  let error = fsm
    .start_named_timer(&context, "tick", AnyMessage::new(1_u32), false, |_key, _message| Err(SchedulerError::Closed))
    .expect_err("scheduler failure");

  assert!(error.is_source_type::<SchedulerError>());
  assert!(!fsm.is_timer_active("tick"));
}

#[test]
fn default_constructs_empty_fsm() {
  let fsm = Fsm::<ProbeState, usize>::default();

  assert!(fsm.state_name().is_none());
}

#[test]
#[should_panic(expected = "Fsm: state timeout must be positive")]
fn set_state_timeout_rejects_zero_duration() {
  let mut fsm = Fsm::<ProbeState, usize>::new();

  fsm.set_state_timeout(ProbeState::Idle, Duration::ZERO);
}
