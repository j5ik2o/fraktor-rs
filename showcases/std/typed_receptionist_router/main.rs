#![cfg(not(target_os = "none"))]

use core::hint::spin_loop;
use std::vec::Vec;

use fraktor_actor_core_rs::core::{
  kernel::actor::scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
  typed::{
    ActorTags, SupervisorStrategy, TypedActorSystem, TypedProps,
    dsl::{Behaviors, routing::Routers},
    receptionist::{Receptionist, ServiceKey},
  },
};
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}

fn main() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let system = TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system");

  let key = ServiceKey::<u32>::new("typed-receptionist-router-example");
  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  let receptionist = Receptionist::get(&system);
  let mut receptionist_ref = receptionist.r#ref();

  let routee_props = ActorTags::new(["example", "routee"]).apply_to(TypedProps::<u32>::from_behavior_factory({
    let records = records.clone();
    move || {
      Behaviors::supervise(Behaviors::receive_message({
        let records = records.clone();
        move |_ctx, message: &u32| {
          records.lock().push(*message);
          Ok(Behaviors::same())
        }
      }))
      .on_failure(SupervisorStrategy::restart())
    }
  }));
  let routee_ref = system.system_actor_of(&routee_props, "typed-receptionist-router-routee").expect("spawn routee");
  receptionist_ref.tell(Receptionist::register(&key, routee_ref));

  let router_props = TypedProps::<u32>::from_behavior_factory({
    let key = key.clone();
    move || Routers::group(key.clone())
  });
  let mut router_ref = system.system_actor_of(&router_props, "typed-receptionist-router").expect("spawn router");

  router_ref.tell(42);
  wait_until(|| records.lock().as_slice() == [42]);

  system.terminate().expect("terminate");
}
