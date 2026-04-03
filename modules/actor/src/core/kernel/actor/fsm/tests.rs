use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use super::{AbstractFsm, Fsm, FsmReason, FsmStateTimeout, FsmTransition, LoggingFsm};
use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  event::stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  system::ActorSystem,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ProbeState {
  Idle,
  Active,
}

#[derive(Clone)]
struct Advance;

#[derive(Clone)]
struct Finish;

struct NoopActor;

impl Actor for NoopActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContext<'_>,
    _message: AnyMessageView<'_>,
  ) -> Result<(), crate::core::kernel::actor::error::ActorError> {
    Ok(())
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
  let props = Props::from_fn(|| NoopActor);
  let _cell = register_cell(&system, pid, "fsm-probe", &props);
  (system.clone(), ActorContext::new(&system, pid))
}

#[test]
fn fsm_transitions_and_reports_transition_callback() {
  let (_system, mut ctx) = build_context();
  let transitions = ArcShared::new(NoStdMutex::new(Vec::new()));
  let transitions_for_cb = transitions.clone();

  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, _data| {
    if message.downcast_ref::<Advance>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active).using(1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.on_transition(move |from, to| {
    transitions_for_cb.lock().push((from.clone(), to.clone()));
  });
  fsm.initialize(&ctx).expect("initialize");

  let message = AnyMessage::new(Advance);
  let view = message.as_view();
  fsm.handle(&mut ctx, &view).expect("handle");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.state_data(), Some(&1));
  assert_eq!(transitions.lock().as_slice(), &[(ProbeState::Idle, ProbeState::Active)]);
}

#[test]
fn fsm_stay_using_updates_data_without_state_change() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 1);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if let Some(delta) = message.downcast_ref::<usize>() {
      return Ok(FsmTransition::stay().using(*data + *delta));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");

  let message = AnyMessage::new(5usize);
  let view = message.as_view();
  fsm.handle(&mut ctx, &view).expect("handle");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));
  assert_eq!(fsm.state_data(), Some(&6));
}

#[test]
fn fsm_stop_records_reason_and_invokes_termination_callback() {
  let (_system, mut ctx) = build_context();
  let terminations = ArcShared::new(NoStdMutex::new(Vec::new()));
  let terminations_for_cb = terminations.clone();

  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Active, 5);
  fsm.when(ProbeState::Active, |_ctx, message: &AnyMessageView<'_>, _state, _data| {
    if message.downcast_ref::<Finish>().is_some() {
      return Ok(FsmTransition::stop(FsmReason::Normal).using(9));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.on_termination(move |reason, state, data| {
    terminations_for_cb.lock().push((reason.clone(), state.clone(), *data));
  });
  fsm.initialize(&ctx).expect("initialize");

  let message = AnyMessage::new(Finish);
  let view = message.as_view();
  fsm.handle(&mut ctx, &view).expect("handle");

  assert!(fsm.is_terminated());
  assert_eq!(fsm.last_stop_reason(), Some(&FsmReason::Normal));
  assert_eq!(terminations.lock().as_slice(), &[(FsmReason::Normal, ProbeState::Active, 9)]);
}

#[test]
fn fsm_state_timeout_message_transitions_when_generation_matches() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.set_state_timeout(ProbeState::Idle, Duration::from_millis(20));
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<FsmStateTimeout<ProbeState>>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");

  let timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, 1));
  let view = timeout.as_view();
  fsm.handle(&mut ctx, &view).expect("timeout");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.state_data(), Some(&1));
}

struct LogRecorder {
  messages: ArcShared<NoStdMutex<Vec<String>>>,
}

impl LogRecorder {
  fn new(messages: ArcShared<NoStdMutex<Vec<String>>>) -> Self {
    Self { messages }
  }
}

impl EventStreamSubscriber for LogRecorder {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Log(log) = event {
      self.messages.lock().push(log.message().to_owned());
    }
  }
}

#[test]
fn logging_fsm_emits_transition_and_termination_logs() {
  let (system, mut ctx) = build_context();
  let logs = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(LogRecorder::new(logs.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);

  let mut fsm = LoggingFsm::<ProbeState, usize>::new().with_logger_name("fsm.test");
  fsm.start_with(ProbeState::Idle, 0);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, _data| {
    if message.downcast_ref::<Advance>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.when(ProbeState::Active, |_ctx, message: &AnyMessageView<'_>, _state, _data| {
    if message.downcast_ref::<Finish>().is_some() {
      return Ok(FsmTransition::stop(FsmReason::Normal));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");

  let advance = AnyMessage::new(Advance);
  let advance_view = advance.as_view();
  fsm.handle(&mut ctx, &advance_view).expect("advance");
  let finish = AnyMessage::new(Finish);
  let finish_view = finish.as_view();
  fsm.handle(&mut ctx, &finish_view).expect("finish");

  let snapshot = logs.lock().clone();

  assert!(snapshot.iter().any(|message| message.contains("fsm transition")));
  assert!(snapshot.iter().any(|message| message.contains("fsm terminated")));
  assert!(snapshot.iter().any(|message| message.contains("Active")));
}

#[test]
fn abstract_fsm_delegates_to_inner_runtime() {
  let (_system, mut ctx) = build_context();
  let mut fsm = AbstractFsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 10);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<usize>().is_some() {
      return Ok(FsmTransition::stay().using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");

  let message = AnyMessage::new(1usize);
  let view = message.as_view();
  fsm.handle(&mut ctx, &view).expect("handle");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));
  assert_eq!(fsm.state_data(), Some(&11));
}
