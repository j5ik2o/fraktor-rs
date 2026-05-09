use std::{thread, time::Duration, vec::Vec};

use fraktor_actor_adaptor_std_rs::std::tick_driver::StdTickDriver;
use fraktor_actor_core_kernel_rs::actor::setup::ActorSystemConfig;
use fraktor_actor_core_typed_rs::{
  ActorTags, SupervisorStrategy, TypedActorSystem, TypedProps,
  dsl::{Behaviors, routing::Routers},
  receptionist::{Receptionist, ServiceKey},
};
use fraktor_utils_core_rs::core::sync::{SharedLock, SpinSyncMutex};

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..1_000 {
    if condition() {
      return;
    }
    thread::sleep(Duration::from_millis(1));
  }
  assert!(condition());
}

fn main() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let system =
    TypedActorSystem::<u32>::create_from_props(&guardian_props, ActorSystemConfig::new(StdTickDriver::default()))
      .expect("system");

  let key = ServiceKey::<u32>::new("typed-receptionist-router-example");
  let records = SharedLock::new_with_driver::<SpinSyncMutex<_>>(Vec::new());
  let receptionist = Receptionist::get(&system);
  let mut receptionist_ref = receptionist.r#ref();

  let routee_props = ActorTags::new(["example", "routee"]).apply_to(TypedProps::<u32>::from_behavior_factory({
    let records = records.clone();
    move || {
      Behaviors::supervise(Behaviors::receive_message({
        let records = records.clone();
        move |_ctx, message: &u32| {
          records.with_lock(|records| records.push(*message));
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
  wait_until(|| records.with_lock(|records| records.as_slice() == [42]));
  println!("typed_receptionist_router delivered records: {:?}", records.with_lock(|records| records.clone()));

  system.terminate().expect("terminate");
}
