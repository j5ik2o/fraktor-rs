use core::time::Duration;
use std::thread;

use fraktor_actor_adaptor_std_rs::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  routing::{RoundRobinRoutingLogic, Routee, Router},
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{SharedLock, SpinSyncMutex};

struct Start;
struct Work(u32);

struct RouterGuardian {
  records: SharedLock<Vec<(usize, u32)>>,
}

impl Actor for RouterGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_none() {
      return Ok(());
    }

    let first = spawn_routee(ctx, self.records.clone(), 0)?;
    let second = spawn_routee(ctx, self.records.clone(), 1)?;
    let mut router = Router::new(RoundRobinRoutingLogic::new(), vec![
      Routee::ActorRef(first.into_actor_ref()),
      Routee::ActorRef(second.into_actor_ref()),
    ]);
    for value in 0..4_u32 {
      router.route(AnyMessage::new(Work(value))).map_err(|error| ActorError::recoverable(format!("{error:?}")))?;
    }
    Ok(())
  }
}

struct RouteeActor {
  index:   usize,
  records: SharedLock<Vec<(usize, u32)>>,
}

impl Actor for RouteeActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(work) = message.downcast_ref::<Work>() {
      self.records.with_lock(|records| records.push((self.index, work.0)));
    }
    Ok(())
  }
}

fn spawn_routee(
  ctx: &mut ActorContext<'_>,
  records: SharedLock<Vec<(usize, u32)>>,
  index: usize,
) -> Result<fraktor_actor_core_kernel_rs::actor::ChildRef, ActorError> {
  let props = Props::from_fn(move || RouteeActor { index, records: records.clone() });
  ctx.spawn_child(&props).map_err(|error| ActorError::recoverable(format!("spawn routee failed: {error:?}")))
}

fn main() {
  let records = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let props = Props::from_fn({
    let records = records.clone();
    move || RouterGuardian { records: records.clone() }
  });
  let system =
    ActorSystem::create_from_props(&props, ActorSystemConfig::new(StdTickDriver::default())).expect("system");
  let termination = system.when_terminated();

  system.user_guardian_ref().tell(AnyMessage::new(Start));
  wait_until(|| records.with_lock(|records| records.len() == 4));
  let snapshot = records.with_lock(|records| records.clone());
  assert_eq!(snapshot.iter().filter(|(index, _)| *index == 0).count(), 2);
  assert_eq!(snapshot.iter().filter(|(index, _)| *index == 1).count(), 2);
  println!("kernel_routing routed {} work items: {snapshot:?}", snapshot.len());

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
