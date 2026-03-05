use core::hint::spin_loop;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::typed::{
  Behaviors, Listing, Receptionist, ReceptionistCommand, ServiceKey, TypedActorSystem, TypedProps, actor::TypedActorRef,
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

fn new_test_system() -> TypedActorSystem<u32> {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system")
}

fn find_listing(receptionist: &mut TypedActorRef<ReceptionistCommand>, key: &ServiceKey<u32>) -> Listing {
  let response = receptionist.ask::<Listing, _>(|reply_to| Receptionist::find(key, reply_to)).expect("ask find");
  let mut future = response.future().clone();
  wait_until(|| future.is_ready());
  future.try_take().expect("find result").expect("listing payload")
}

#[test]
fn behavior_should_be_constructible() {
  let _behavior = Receptionist::behavior();
}

#[test]
fn unsubscribe_should_stop_listing_updates() {
  let system = new_test_system();
  let mut receptionist = system.receptionist_ref().expect("system receptionist");
  let key = ServiceKey::<u32>::new("svc");

  let updates = ArcShared::new(NoStdMutex::new(0_usize));
  let subscriber_props = TypedProps::<Listing>::from_behavior_factory({
    let updates = updates.clone();
    move || {
      let updates = updates.clone();
      Behaviors::receive_message(move |_ctx, _listing: &Listing| {
        *updates.lock() += 1;
        Ok(Behaviors::same())
      })
    }
  });
  let subscriber = system.as_untyped().spawn(subscriber_props.to_untyped()).expect("spawn listing subscriber");
  let subscriber_ref = TypedActorRef::<Listing>::from_untyped(subscriber.actor_ref().clone());

  receptionist.tell(Receptionist::subscribe(&key, subscriber_ref.clone())).expect("subscribe receptionist");
  wait_until(|| *updates.lock() == 1);

  receptionist.tell(Receptionist::unsubscribe(&key, subscriber_ref)).expect("unsubscribe receptionist");

  let routee_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.actor_ref().clone());
  receptionist.tell(Receptionist::register(&key, routee_ref)).expect("register routee");

  for _ in 0..10_000 {
    assert_eq!(*updates.lock(), 1);
    spin_loop();
  }

  system.terminate().expect("terminate");
}

#[test]
fn terminated_routee_should_be_removed_from_listing() {
  let system = new_test_system();
  let mut receptionist = system.receptionist_ref().expect("system receptionist");
  let key = ServiceKey::<u32>::new("svc");

  let routee_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.actor_ref().clone());
  receptionist.tell(Receptionist::register(&key, routee_ref)).expect("register routee");

  wait_until(|| !find_listing(&mut receptionist, &key).is_empty());

  routee.stop().expect("stop routee");
  wait_until(|| find_listing(&mut receptionist, &key).is_empty());

  system.terminate().expect("terminate");
}

#[test]
fn terminated_subscriber_should_be_cleaned_up() {
  let system = new_test_system();
  let mut receptionist = system.receptionist_ref().expect("system receptionist");
  let key = ServiceKey::<u32>::new("svc");

  let updates = ArcShared::new(NoStdMutex::new(0_usize));
  let subscriber_props = TypedProps::<Listing>::from_behavior_factory({
    let updates = updates.clone();
    move || {
      let updates = updates.clone();
      Behaviors::receive_message(move |_ctx, _listing: &Listing| {
        *updates.lock() += 1;
        Ok(Behaviors::same())
      })
    }
  });
  let subscriber = system.as_untyped().spawn(subscriber_props.to_untyped()).expect("spawn listing subscriber");
  let subscriber_ref = TypedActorRef::<Listing>::from_untyped(subscriber.actor_ref().clone());

  receptionist.tell(Receptionist::subscribe(&key, subscriber_ref)).expect("subscribe receptionist");
  wait_until(|| *updates.lock() == 1);

  subscriber.stop().expect("stop listing subscriber");

  let routee_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.actor_ref().clone());
  receptionist.tell(Receptionist::register(&key, routee_ref)).expect("register routee");

  for _ in 0..10_000 {
    assert_eq!(*updates.lock(), 1);
    spin_loop();
  }

  system.terminate().expect("terminate");
}
