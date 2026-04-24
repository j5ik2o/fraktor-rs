use alloc::{string::String, vec::Vec};
use core::time::Duration;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::{Fsm, FsmReason, FsmStateTimeout, FsmTimerFired, FsmTransition, LoggingFsm};
use crate::core::kernel::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_ref::{ActorRef, ActorRefSender, SendOutcome, dead_letter::DeadLetterReason},
    error::{ActorError, SendError},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  event::stream::{EventStreamEvent, EventStreamSubscriber, tests::subscriber_handle},
  system::ActorSystem,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ProbeState {
  Idle,
  Active,
  Waiting,
}

#[derive(Clone)]
struct Advance;

#[derive(Clone)]
struct Finish;

#[derive(Clone)]
struct Ignore;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Reply(&'static str);

#[derive(Clone, Debug, PartialEq, Eq)]
struct TimerPayload(&'static str);

struct NoopActor;

struct CapturingSender {
  inbox: ArcShared<SpinSyncMutex<Vec<AnyMessage>>>,
}

impl ActorRefSender for CapturingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    self.inbox.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
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

fn capturing_sender(pid: Pid) -> (ArcShared<SpinSyncMutex<Vec<AnyMessage>>>, ActorRef) {
  let inbox = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let sender = ActorRef::new_with_builtin_lock(pid, CapturingSender { inbox: inbox.clone() });
  (inbox, sender)
}

#[test]
fn fsm_transitions_and_reports_transition_callback() {
  let (_system, mut ctx) = build_context();
  let transitions = ArcShared::new(SpinSyncMutex::new(Vec::new()));
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
fn fsm_stay_does_not_reschedule_state_timeout() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 1);
  fsm.set_state_timeout(ProbeState::Idle, Duration::from_millis(20));
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if let Some(delta) = message.downcast_ref::<usize>() {
      return Ok(FsmTransition::stay().using(*data + *delta));
    }
    if message.downcast_ref::<FsmStateTimeout<ProbeState>>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");
  let generation = fsm.generation();

  let stay = AnyMessage::new(5usize);
  let stay_view = stay.as_view();
  fsm.handle(&mut ctx, &stay_view).expect("stay");

  assert_eq!(fsm.generation(), generation);
  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));
  assert_eq!(fsm.state_data(), Some(&6));

  let timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, generation));
  let timeout_view = timeout.as_view();
  fsm.handle(&mut ctx, &timeout_view).expect("timeout");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.state_data(), Some(&7));
}

#[test]
fn fsm_goto_same_state_rearms_timeout_and_notifies_transition_observer() {
  let (_system, mut ctx) = build_context();
  let transitions = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let transitions_for_cb = transitions.clone();

  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 1);
  fsm.set_state_timeout(ProbeState::Idle, Duration::from_millis(20));
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if let Some(delta) = message.downcast_ref::<usize>() {
      return Ok(FsmTransition::goto(ProbeState::Idle).using(*data + *delta));
    }
    if message.downcast_ref::<FsmStateTimeout<ProbeState>>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.on_transition(move |from, to| {
    transitions_for_cb.lock().push((from.clone(), to.clone()));
  });
  fsm.initialize(&ctx).expect("initialize");
  let generation = fsm.generation();

  let reenter = AnyMessage::new(5usize);
  let reenter_view = reenter.as_view();
  fsm.handle(&mut ctx, &reenter_view).expect("reenter");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));
  assert_eq!(fsm.state_data(), Some(&6));
  assert_eq!(fsm.generation(), generation + 1);
  assert_eq!(transitions.lock().as_slice(), &[(ProbeState::Idle, ProbeState::Idle)]);

  let stale_timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, generation));
  let stale_timeout_view = stale_timeout.as_view();
  fsm.handle(&mut ctx, &stale_timeout_view).expect("stale timeout");
  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));

  let timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, generation + 1));
  let timeout_view = timeout.as_view();
  fsm.handle(&mut ctx, &timeout_view).expect("timeout");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.state_data(), Some(&7));
}

#[test]
fn fsm_stop_records_reason_and_invokes_termination_callback() {
  let (_system, mut ctx) = build_context();
  let terminations = ArcShared::new(SpinSyncMutex::new(Vec::new()));
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
fn fsm_restart_requires_initialize_before_handling_messages() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Active, 5);
  fsm.when(ProbeState::Active, |_ctx, message: &AnyMessageView<'_>, _state, _data| {
    if message.downcast_ref::<Finish>().is_some() {
      return Ok(FsmTransition::stop(FsmReason::Normal));
    }
    if message.downcast_ref::<Advance>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Idle).using(0));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");

  let finish = AnyMessage::new(Finish);
  let finish_view = finish.as_view();
  fsm.handle(&mut ctx, &finish_view).expect("finish");
  assert!(fsm.is_terminated());

  fsm.start_with(ProbeState::Active, 1);
  let advance = AnyMessage::new(Advance);
  let advance_view = advance.as_view();
  let error = fsm.handle(&mut ctx, &advance_view).expect_err("restart should require initialize");

  assert!(!fsm.is_terminated());
  assert!(matches!(error, ActorError::Recoverable(reason) if reason.as_str() == "fsm not initialized"));

  fsm.initialize(&ctx).expect("reinitialize");
  fsm.handle(&mut ctx, &advance_view).expect("advance");
  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));
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
  let generation = fsm.generation();

  let timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, generation));
  let view = timeout.as_view();
  fsm.handle(&mut ctx, &view).expect("timeout");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.state_data(), Some(&1));
}

#[test]
fn fsm_no_timeout_transition_does_not_bump_generation_before_timed_state() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.set_state_timeout(ProbeState::Waiting, Duration::from_millis(20));
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<Advance>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.when(ProbeState::Active, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<Finish>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Waiting).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.when(ProbeState::Waiting, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<FsmStateTimeout<ProbeState>>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Idle).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");
  assert_eq!(fsm.generation(), 0);

  let advance = AnyMessage::new(Advance);
  let advance_view = advance.as_view();
  fsm.handle(&mut ctx, &advance_view).expect("advance");
  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.generation(), 0);

  let finish = AnyMessage::new(Finish);
  let finish_view = finish.as_view();
  fsm.handle(&mut ctx, &finish_view).expect("finish");
  assert_eq!(fsm.state_name(), Some(&ProbeState::Waiting));
  assert_eq!(fsm.generation(), 1);

  let timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Waiting, 1));
  let timeout_view = timeout.as_view();
  fsm.handle(&mut ctx, &timeout_view).expect("timeout");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));
  assert_eq!(fsm.state_data(), Some(&3));
}

#[test]
fn fsm_unhandled_message_does_not_reschedule_state_timeout() {
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
  let generation = fsm.generation();

  let ignore = AnyMessage::new(Ignore);
  let ignore_view = ignore.as_view();
  fsm.handle(&mut ctx, &ignore_view).expect("ignore");

  assert_eq!(fsm.generation(), generation);

  let timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, generation));
  let timeout_view = timeout.as_view();
  fsm.handle(&mut ctx, &timeout_view).expect("timeout");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.state_data(), Some(&1));
}

#[test]
fn for_max_some_installs_transient_timeout() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.set_state_timeout(ProbeState::Active, Duration::from_millis(50));
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<Advance>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1).for_max(Some(Duration::from_millis(5))));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.when(ProbeState::Active, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<FsmStateTimeout<ProbeState>>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Waiting).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");

  let advance = AnyMessage::new(Advance);
  let advance_view = advance.as_view();
  fsm.handle(&mut ctx, &advance_view).expect("advance");
  let generation = fsm.generation();

  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(generation, 1);

  let timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Active, generation));
  let timeout_view = timeout.as_view();
  fsm.handle(&mut ctx, &timeout_view).expect("timeout");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Waiting));
  assert_eq!(fsm.state_data(), Some(&2));
}

#[test]
fn for_max_none_cancels_state_timeout() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.set_state_timeout(ProbeState::Idle, Duration::from_millis(20));
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<Advance>().is_some() {
      return Ok(FsmTransition::stay().using(*data + 1).for_max(None));
    }
    if message.downcast_ref::<Finish>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Idle).using(*data + 1));
    }
    if message.downcast_ref::<FsmStateTimeout<ProbeState>>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");
  let initial_generation = fsm.generation();

  let advance = AnyMessage::new(Advance);
  let advance_view = advance.as_view();
  fsm.handle(&mut ctx, &advance_view).expect("advance");

  assert_eq!(fsm.generation(), initial_generation + 1);

  let stale_timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, initial_generation));
  let stale_timeout_view = stale_timeout.as_view();
  fsm.handle(&mut ctx, &stale_timeout_view).expect("stale timeout");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));
  assert_eq!(fsm.state_data(), Some(&1));

  let finish = AnyMessage::new(Finish);
  let finish_view = finish.as_view();
  fsm.handle(&mut ctx, &finish_view).expect("finish");
  let restored_generation = fsm.generation();

  let timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, restored_generation));
  let timeout_view = timeout.as_view();
  fsm.handle(&mut ctx, &timeout_view).expect("timeout");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.state_data(), Some(&3));
}

#[test]
fn stay_applies_for_max_override() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.set_state_timeout(ProbeState::Idle, Duration::from_millis(20));
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<Advance>().is_some() {
      return Ok(FsmTransition::stay().using(*data + 1).for_max(Some(Duration::from_millis(2))));
    }
    if message.downcast_ref::<FsmStateTimeout<ProbeState>>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");
  let initial_generation = fsm.generation();

  let advance = AnyMessage::new(Advance);
  let advance_view = advance.as_view();
  fsm.handle(&mut ctx, &advance_view).expect("advance");
  let override_generation = fsm.generation();

  assert_eq!(override_generation, initial_generation + 1);

  let stale_timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, initial_generation));
  let stale_timeout_view = stale_timeout.as_view();
  fsm.handle(&mut ctx, &stale_timeout_view).expect("stale timeout");
  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));

  let timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, override_generation));
  let timeout_view = timeout.as_view();
  fsm.handle(&mut ctx, &timeout_view).expect("timeout");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.state_data(), Some(&2));
}

#[test]
fn state_timeouts_reapplies_after_for_max_override() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.set_state_timeout(ProbeState::Idle, Duration::from_millis(20));
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<Advance>().is_some() {
      return Ok(FsmTransition::stay().using(*data + 1).for_max(Some(Duration::from_millis(2))));
    }
    if message.downcast_ref::<Finish>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Idle).using(*data + 1));
    }
    if message.downcast_ref::<FsmStateTimeout<ProbeState>>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");

  let advance = AnyMessage::new(Advance);
  let advance_view = advance.as_view();
  fsm.handle(&mut ctx, &advance_view).expect("advance");
  let override_generation = fsm.generation();

  let finish = AnyMessage::new(Finish);
  let finish_view = finish.as_view();
  fsm.handle(&mut ctx, &finish_view).expect("finish");
  let restored_generation = fsm.generation();

  assert_eq!(restored_generation, override_generation + 1);

  let stale_timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, override_generation));
  let stale_timeout_view = stale_timeout.as_view();
  fsm.handle(&mut ctx, &stale_timeout_view).expect("stale timeout");
  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));

  let timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, restored_generation));
  let timeout_view = timeout.as_view();
  fsm.handle(&mut ctx, &timeout_view).expect("timeout");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.state_data(), Some(&3));
}

#[test]
fn for_max_zero_duration_normalized_to_cancel() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.set_state_timeout(ProbeState::Idle, Duration::from_millis(20));
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<Advance>().is_some() {
      return Ok(FsmTransition::stay().using(*data + 1).for_max(Some(Duration::ZERO)));
    }
    if message.downcast_ref::<FsmStateTimeout<ProbeState>>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");
  let initial_generation = fsm.generation();

  let advance = AnyMessage::new(Advance);
  let advance_view = advance.as_view();
  fsm.handle(&mut ctx, &advance_view).expect("advance");

  assert_eq!(fsm.generation(), initial_generation + 1);

  let stale_timeout = AnyMessage::new(FsmStateTimeout::new(ProbeState::Idle, initial_generation));
  let stale_timeout_view = stale_timeout.as_view();
  fsm.handle(&mut ctx, &stale_timeout_view).expect("stale timeout");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));
  assert_eq!(fsm.state_data(), Some(&1));
}

#[test]
fn replying_basic_delivers_to_sender() {
  let (system, mut ctx) = build_context();
  let (inbox, sender_ref) = capturing_sender(system.allocate_pid());
  ctx.set_sender(Some(sender_ref));

  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, _data| {
    if message.downcast_ref::<Advance>().is_some() {
      return Ok(FsmTransition::stay().replying(AnyMessage::new(Reply("ack"))));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");

  let advance = AnyMessage::new(Advance);
  let advance_view = advance.as_view();
  fsm.handle(&mut ctx, &advance_view).expect("advance");

  let captured = inbox.lock();
  assert_eq!(captured.len(), 1);
  assert_eq!(captured[0].downcast_ref::<Reply>(), Some(&Reply("ack")));
}

#[test]
fn replying_multiple_preserves_order() {
  let (system, mut ctx) = build_context();
  let (inbox, sender_ref) = capturing_sender(system.allocate_pid());
  ctx.set_sender(Some(sender_ref));

  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, _data| {
    if message.downcast_ref::<Advance>().is_some() {
      return Ok(
        FsmTransition::stay().replying(AnyMessage::new(Reply("first"))).replying(AnyMessage::new(Reply("second"))),
      );
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");

  let advance = AnyMessage::new(Advance);
  let advance_view = advance.as_view();
  fsm.handle(&mut ctx, &advance_view).expect("advance");

  let captured = inbox.lock();

  assert_eq!(captured.len(), 2);
  assert_eq!(captured[0].downcast_ref::<Reply>(), Some(&Reply("first")));
  assert_eq!(captured[1].downcast_ref::<Reply>(), Some(&Reply("second")));
}

#[test]
fn replying_without_sender_records_missing_recipient_dead_letter() {
  let (system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, _data| {
    if message.downcast_ref::<Advance>().is_some() {
      return Ok(FsmTransition::stay().replying(AnyMessage::new(Reply("missing"))));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");

  let advance = AnyMessage::new(Advance);
  let advance_view = advance.as_view();
  fsm.handle(&mut ctx, &advance_view).expect("advance");

  let dead_letters = system.dead_letters();
  assert_eq!(dead_letters.len(), 1);
  assert_eq!(dead_letters[0].reason(), DeadLetterReason::MissingRecipient);
  assert_eq!(dead_letters[0].recipient(), None);
  assert_eq!(dead_letters[0].message().downcast_ref::<Reply>(), Some(&Reply("missing")));
}

#[test]
fn start_single_timer_fires_and_unwraps_payload() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if let Some(payload) = message.downcast_ref::<TimerPayload>()
      && payload == &TimerPayload("once")
    {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");
  fsm
    .start_single_timer(&mut ctx, "tick", AnyMessage::new(TimerPayload("once")), Duration::from_millis(10))
    .expect("start timer");

  assert!(fsm.is_timer_active("tick"));

  let fired = AnyMessage::new(FsmTimerFired::new(String::from("tick"), 1, AnyMessage::new(TimerPayload("once"))));
  let fired_view = fired.as_view();
  fsm.handle(&mut ctx, &fired_view).expect("timer fired");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.state_data(), Some(&1));
  assert!(!fsm.is_timer_active("tick"));
}

#[test]
fn start_timer_at_fixed_rate_fires_repeatedly() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<TimerPayload>().is_some() {
      return Ok(FsmTransition::stay().using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");
  fsm
    .start_timer_at_fixed_rate(&mut ctx, "tick", AnyMessage::new(TimerPayload("rate")), Duration::from_millis(10))
    .expect("start timer");

  let fired = AnyMessage::new(FsmTimerFired::new(String::from("tick"), 1, AnyMessage::new(TimerPayload("rate"))));
  let fired_view = fired.as_view();
  fsm.handle(&mut ctx, &fired_view).expect("timer fired 1");
  fsm.handle(&mut ctx, &fired_view).expect("timer fired 2");

  assert_eq!(fsm.state_data(), Some(&2));
  assert!(fsm.is_timer_active("tick"));
}

#[test]
fn start_timer_with_fixed_delay_fires_repeatedly() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<TimerPayload>().is_some() {
      return Ok(FsmTransition::stay().using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");
  fsm
    .start_timer_with_fixed_delay(&mut ctx, "tick", AnyMessage::new(TimerPayload("delay")), Duration::from_millis(10))
    .expect("start timer");

  let fired = AnyMessage::new(FsmTimerFired::new(String::from("tick"), 1, AnyMessage::new(TimerPayload("delay"))));
  let fired_view = fired.as_view();
  fsm.handle(&mut ctx, &fired_view).expect("timer fired 1");
  fsm.handle(&mut ctx, &fired_view).expect("timer fired 2");

  assert_eq!(fsm.state_data(), Some(&2));
  assert!(fsm.is_timer_active("tick"));
}

#[test]
fn cancel_timer_prevents_fire() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<TimerPayload>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");
  fsm
    .start_single_timer(&mut ctx, "tick", AnyMessage::new(TimerPayload("cancelled")), Duration::from_millis(10))
    .expect("start timer");
  fsm.cancel_timer(&ctx, "tick").expect("cancel timer");

  let fired = AnyMessage::new(FsmTimerFired::new(String::from("tick"), 1, AnyMessage::new(TimerPayload("cancelled"))));
  let fired_view = fired.as_view();
  fsm.handle(&mut ctx, &fired_view).expect("timer fired");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));
  assert_eq!(fsm.state_data(), Some(&0));
  assert!(!fsm.is_timer_active("tick"));
}

#[test]
fn is_timer_active_tracks_lifecycle() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<TimerPayload>().is_some() {
      return Ok(FsmTransition::stay().using(*data + 1));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");

  assert!(!fsm.is_timer_active("tick"));
  fsm
    .start_single_timer(&mut ctx, "tick", AnyMessage::new(TimerPayload("once")), Duration::from_millis(10))
    .expect("start single");
  assert!(fsm.is_timer_active("tick"));

  let single = AnyMessage::new(FsmTimerFired::new(String::from("tick"), 1, AnyMessage::new(TimerPayload("once"))));
  let single_view = single.as_view();
  fsm.handle(&mut ctx, &single_view).expect("single fired");
  assert!(!fsm.is_timer_active("tick"));

  fsm
    .start_timer_at_fixed_rate(&mut ctx, "tick", AnyMessage::new(TimerPayload("rate")), Duration::from_millis(10))
    .expect("start repeating");
  assert!(fsm.is_timer_active("tick"));
  fsm.cancel_timer(&ctx, "tick").expect("cancel repeating");
  assert!(!fsm.is_timer_active("tick"));
}

#[test]
fn restart_same_name_discards_late_arrival() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if let Some(payload) = message.downcast_ref::<TimerPayload>()
      && payload == &TimerPayload("new")
    {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 1));
    }
    if message.downcast_ref::<TimerPayload>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Waiting).using(*data + 10));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");
  fsm
    .start_single_timer(&mut ctx, "tick", AnyMessage::new(TimerPayload("old")), Duration::from_millis(10))
    .expect("start old");
  fsm
    .start_single_timer(&mut ctx, "tick", AnyMessage::new(TimerPayload("new")), Duration::from_millis(10))
    .expect("start new");

  let old = AnyMessage::new(FsmTimerFired::new(String::from("tick"), 1, AnyMessage::new(TimerPayload("old"))));
  let old_view = old.as_view();
  fsm.handle(&mut ctx, &old_view).expect("old fired");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));
  assert_eq!(fsm.state_data(), Some(&0));
  assert!(fsm.is_timer_active("tick"));

  let new = AnyMessage::new(FsmTimerFired::new(String::from("tick"), 2, AnyMessage::new(TimerPayload("new"))));
  let new_view = new.as_view();
  fsm.handle(&mut ctx, &new_view).expect("new fired");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Active));
  assert_eq!(fsm.state_data(), Some(&1));
  assert!(!fsm.is_timer_active("tick"));
}

#[test]
fn stop_cancels_all_named_timers() {
  let (_system, mut ctx) = build_context();
  let mut fsm = Fsm::<ProbeState, usize>::new();
  fsm.start_with(ProbeState::Idle, 0);
  fsm.when(ProbeState::Idle, |_ctx, message: &AnyMessageView<'_>, _state, data| {
    if message.downcast_ref::<Finish>().is_some() {
      return Ok(FsmTransition::stop(FsmReason::Normal).using(*data + 1));
    }
    if message.downcast_ref::<TimerPayload>().is_some() {
      return Ok(FsmTransition::goto(ProbeState::Active).using(*data + 10));
    }
    Ok(FsmTransition::unhandled())
  });
  fsm.initialize(&ctx).expect("initialize");
  fsm
    .start_timer_with_fixed_delay(&mut ctx, "tick", AnyMessage::new(TimerPayload("delay")), Duration::from_millis(10))
    .expect("start timer");
  assert!(fsm.is_timer_active("tick"));

  let finish = AnyMessage::new(Finish);
  let finish_view = finish.as_view();
  fsm.handle(&mut ctx, &finish_view).expect("finish");

  assert!(fsm.is_terminated());
  assert_eq!(fsm.state_data(), Some(&1));
  assert!(!fsm.is_timer_active("tick"));

  let fired = AnyMessage::new(FsmTimerFired::new(String::from("tick"), 1, AnyMessage::new(TimerPayload("delay"))));
  let fired_view = fired.as_view();
  fsm.handle(&mut ctx, &fired_view).expect("late timer fired");

  assert_eq!(fsm.state_name(), Some(&ProbeState::Idle));
  assert_eq!(fsm.state_data(), Some(&1));
}

struct LogRecorder {
  messages: ArcShared<SpinSyncMutex<Vec<String>>>,
}

impl LogRecorder {
  fn new(messages: ArcShared<SpinSyncMutex<Vec<String>>>) -> Self {
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
  let logs = ArcShared::new(SpinSyncMutex::new(Vec::new()));
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
