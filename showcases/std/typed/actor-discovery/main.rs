use core::time::Duration;
use std::{thread, time::Instant};

use fraktor_actor_adaptor_std_rs::std::{StdBlocker, tick_driver::StdTickDriver};
use fraktor_actor_core_rs::core::{
  kernel::actor::setup::ActorSystemConfig,
  typed::{
    TypedActorSystem, TypedProps,
    dsl::Behaviors,
    receptionist::{Listing, Receptionist, ServiceKey},
  },
};

fn main() {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let system = TypedActorSystem::create_with_config(&guardian_props, ActorSystemConfig::new(StdTickDriver::default()))
    .expect("system");
  let termination = system.when_terminated();
  let key = ServiceKey::<u32>::new("typed-discovery-example");
  let service_ref = system
    .system_actor_of(&TypedProps::<u32>::from_behavior_factory(Behaviors::ignore), "typed-discovery-service")
    .expect("spawn service");
  let receptionist = Receptionist::get(&system);
  let mut receptionist_ref = receptionist.r#ref();

  receptionist_ref.tell(Receptionist::register(&key, service_ref.clone()));
  let listing = wait_for_listing(&mut receptionist_ref, &key);
  let instances = listing.service_instances(&key).expect("matching key");
  assert!(instances.contains(&service_ref));
  println!("typed_actor_discovery found {} service instance(s)", instances.len());

  system.terminate().expect("terminate");
  termination.wait_blocking(&StdBlocker::new());
}

fn wait_for_listing(
  receptionist_ref: &mut fraktor_actor_core_rs::core::typed::TypedActorRef<
    fraktor_actor_core_rs::core::typed::receptionist::ReceptionistCommand,
  >,
  key: &ServiceKey<u32>,
) -> Listing {
  let deadline = Instant::now() + Duration::from_secs(5);
  loop {
    let response = receptionist_ref.ask::<Listing, _>(|reply_to| Receptionist::find(key, reply_to));
    let mut future = response.future().clone();
    let ask_deadline = Instant::now() + Duration::from_millis(500);
    while !future.is_ready() && Instant::now() < ask_deadline {
      thread::sleep(Duration::from_millis(1));
    }
    if future.is_ready() {
      let listing = future.try_take().expect("ready").expect("ok");
      if !listing.is_empty() {
        return listing;
      }
    }
    assert!(Instant::now() < deadline, "receptionist listing should contain the registered actor before timeout");
  }
}
