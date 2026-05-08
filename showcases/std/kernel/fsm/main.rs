use core::time::Duration;
use std::thread;

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    fsm::{Fsm, FsmTransition},
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum GateState {
  Locked,
  Open,
}

struct Coin;
struct Pass;

struct GateActor {
  fsm: Fsm<GateState, u32>,
}

impl GateActor {
  fn new(transitions: SharedLock<Vec<&'static str>>, pass_count: SharedLock<u32>) -> Self {
    let mut fsm = Fsm::new();
    fsm.start_with(GateState::Locked, 0);
    fsm.when(GateState::Locked, |_ctx, message, _state, data| {
      if message.downcast_ref::<Coin>().is_some() {
        return Ok(FsmTransition::goto(GateState::Open).using(*data));
      }
      Ok(FsmTransition::unhandled())
    });
    let pass_count_for_handler = pass_count.clone();
    fsm.when(GateState::Open, move |_ctx, message, _state, data| {
      if message.downcast_ref::<Pass>().is_some() {
        let next_count = *data + 1;
        pass_count_for_handler.with_lock(|pass_count| *pass_count = next_count);
        return Ok(FsmTransition::goto(GateState::Locked).using(next_count));
      }
      Ok(FsmTransition::unhandled())
    });
    let transitions_for_observer = transitions.clone();
    fsm.on_transition(move |from, to| {
      let label = match (*from, *to) {
        | (GateState::Locked, GateState::Open) => "locked-to-open",
        | (GateState::Open, GateState::Locked) => "open-to-locked",
        | _ => "other",
      };
      transitions_for_observer.with_lock(|transitions| transitions.push(label));
    });
    Self { fsm }
  }
}

impl Actor for GateActor {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.fsm.initialize(ctx)
  }

  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    self.fsm.handle(ctx, &message)
  }
}

fn main() {
  let transitions = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let pass_count = SharedLock::new_with_driver::<SpinSyncMutex<_>>(0_u32);
  let props = Props::from_fn({
    let transitions = transitions.clone();
    let pass_count = pass_count.clone();
    move || GateActor::new(transitions.clone(), pass_count.clone())
  });
  let system =
    ActorSystem::create_with_config(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();

  let mut guardian = system.user_guardian_ref();
  guardian.tell(AnyMessage::new(Coin));
  guardian.tell(AnyMessage::new(Pass));
  wait_until(|| pass_count.with_lock(|count| *count == 1));
  let transitions_snapshot = transitions.with_lock(|transitions| transitions.clone());
  assert_eq!(transitions_snapshot, vec!["locked-to-open", "open-to-locked"]);
  println!("kernel_fsm completed transitions: {transitions_snapshot:?}");

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..1_000 {
    if condition() {
      return;
    }
    thread::sleep(Duration::from_millis(1));
  }
  assert!(condition());
}
