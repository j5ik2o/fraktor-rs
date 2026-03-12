use core::hint::spin_loop;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::{
  scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
  typed::{Behaviors, TypedActorSystem, TypedProps},
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

#[test]
fn delegate_returns_delegatee_when_behavior_reports_same() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let system =
    TypedActorSystem::<u32>::new(&guardian_props, TickDriverConfig::manual(ManualTestDriver::new())).expect("system");

  let outer_count = ArcShared::new(NoStdMutex::new(0_usize));
  let inner_count = ArcShared::new(NoStdMutex::new(0_usize));
  let actor_props = TypedProps::<u32>::from_behavior_factory({
    let outer_count = outer_count.clone();
    let inner_count = inner_count.clone();
    move || {
      let outer_count = outer_count.clone();
      let inner_count = inner_count.clone();
      Behaviors::receive_message(move |ctx, message: &u32| {
        *outer_count.lock() += 1;
        let inner_count = inner_count.clone();
        let delegated = Behaviors::receive_message(move |_ctx, inner_message: &u32| {
          if *inner_message > 0 {
            *inner_count.lock() += 1;
          }
          Ok(Behaviors::same())
        });
        ctx.delegate(delegated, message)
      })
    }
  });
  let actor = system.as_untyped().spawn(actor_props.to_untyped()).expect("spawn actor");
  let mut actor = crate::core::typed::actor::TypedActorRef::<u32>::from_untyped(actor.actor_ref().clone());

  actor.tell(1).expect("first");
  actor.tell(1).expect("second");
  wait_until(|| *inner_count.lock() == 2);

  assert_eq!(*outer_count.lock(), 1);
  assert_eq!(*inner_count.lock(), 2);
  system.terminate().expect("terminate");
}
