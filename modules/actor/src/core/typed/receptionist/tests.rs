use core::hint::spin_loop;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::typed::{
  TypedActorRef, TypedActorSystem, TypedProps,
  dsl::Behaviors,
  receptionist::{Listing, Receptionist, ReceptionistCommand, ServiceKey},
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
  let tick_driver = crate::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system")
}

fn find_listing(receptionist: &mut TypedActorRef<ReceptionistCommand>, key: &ServiceKey<u32>) -> Listing {
  let response = receptionist.ask::<Listing, _>(|reply_to| Receptionist::find(key, reply_to));
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
  let subscriber_ref = TypedActorRef::<Listing>::from_untyped(subscriber.into_actor_ref());

  receptionist.tell(Receptionist::subscribe(&key, subscriber_ref.clone()));
  wait_until(|| *updates.lock() == 1);

  receptionist.tell(Receptionist::unsubscribe(&key, subscriber_ref));

  let routee_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.clone().into_actor_ref());
  receptionist.tell(Receptionist::register(&key, routee_ref));

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
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.clone().into_actor_ref());
  receptionist.tell(Receptionist::register(&key, routee_ref));

  wait_until(|| !find_listing(&mut receptionist, &key).is_empty());

  routee.stop().expect("stop routee");
  wait_until(|| find_listing(&mut receptionist, &key).is_empty());

  system.terminate().expect("terminate");
}

// --- T6: Receptionist ACK tests ---

#[test]
fn register_with_ack_sends_registered_to_reply_to() {
  // Given: a system with a receptionist and an ack receiver
  let system = new_test_system();
  let mut receptionist = system.receptionist_ref().expect("system receptionist");
  let key = ServiceKey::<u32>::new("ack-svc");

  let ack_received = ArcShared::new(NoStdMutex::new(false));
  let ack_service_id = ArcShared::new(NoStdMutex::new(alloc::string::String::new()));

  // Spawn a receiver for Registered ack
  let ack_props = TypedProps::<crate::core::typed::receptionist::Registered>::from_behavior_factory({
    let ack_received = ack_received.clone();
    let ack_service_id = ack_service_id.clone();
    move || {
      let ack_received = ack_received.clone();
      let ack_service_id = ack_service_id.clone();
      Behaviors::receive_message(move |_ctx, registered: &crate::core::typed::receptionist::Registered| {
        *ack_service_id.lock() = alloc::string::String::from(registered.service_id());
        *ack_received.lock() = true;
        Ok(Behaviors::same())
      })
    }
  });
  let ack_actor = system.as_untyped().spawn(ack_props.to_untyped()).expect("spawn ack receiver");
  let ack_ref = TypedActorRef::<crate::core::typed::receptionist::Registered>::from_untyped(ack_actor.into_actor_ref());

  // Spawn a routee to register
  let routee_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.into_actor_ref());

  // When: register_with_ack is called
  receptionist.tell(Receptionist::register_with_ack(&key, routee_ref, ack_ref));

  // Then: the ack receiver gets a Registered message with the correct service_id
  wait_until(|| *ack_received.lock());
  assert_eq!(ack_service_id.lock().as_str(), "ack-svc");

  system.terminate().expect("terminate");
}

#[test]
fn deregister_with_ack_sends_deregistered_to_reply_to() {
  // Given: a system with a registered actor
  let system = new_test_system();
  let mut receptionist = system.receptionist_ref().expect("system receptionist");
  let key = ServiceKey::<u32>::new("dereg-svc");

  let routee_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.into_actor_ref());

  // First register the routee
  receptionist.tell(Receptionist::register(&key, routee_ref.clone()));
  wait_until(|| !find_listing(&mut receptionist, &key).is_empty());

  // Spawn an ack receiver for Deregistered
  let ack_received = ArcShared::new(NoStdMutex::new(false));
  let ack_service_id = ArcShared::new(NoStdMutex::new(alloc::string::String::new()));

  let ack_props = TypedProps::<crate::core::typed::receptionist::Deregistered>::from_behavior_factory({
    let ack_received = ack_received.clone();
    let ack_service_id = ack_service_id.clone();
    move || {
      let ack_received = ack_received.clone();
      let ack_service_id = ack_service_id.clone();
      Behaviors::receive_message(move |_ctx, deregistered: &crate::core::typed::receptionist::Deregistered| {
        *ack_service_id.lock() = alloc::string::String::from(deregistered.service_id());
        *ack_received.lock() = true;
        Ok(Behaviors::same())
      })
    }
  });
  let ack_actor = system.as_untyped().spawn(ack_props.to_untyped()).expect("spawn ack receiver");
  let ack_ref =
    TypedActorRef::<crate::core::typed::receptionist::Deregistered>::from_untyped(ack_actor.into_actor_ref());

  // When: deregister_with_ack is called
  receptionist.tell(Receptionist::deregister_with_ack(&key, routee_ref, ack_ref));

  // Then: the ack receiver gets a Deregistered message with the correct service_id
  wait_until(|| *ack_received.lock());
  assert_eq!(ack_service_id.lock().as_str(), "dereg-svc");

  system.terminate().expect("terminate");
}

#[test]
fn registered_is_for_key_returns_true_for_matching_key() {
  // Given: a system with a registered actor via register_with_ack
  let system = new_test_system();
  let mut receptionist = system.receptionist_ref().expect("system receptionist");
  let key = ServiceKey::<u32>::new("key-check-svc");

  let captured_registered =
    ArcShared::new(NoStdMutex::new(Option::<crate::core::typed::receptionist::Registered>::None));

  let ack_props = TypedProps::<crate::core::typed::receptionist::Registered>::from_behavior_factory({
    let captured = captured_registered.clone();
    move || {
      let captured = captured.clone();
      Behaviors::receive_message(move |_ctx, registered: &crate::core::typed::receptionist::Registered| {
        *captured.lock() = Some(registered.clone());
        Ok(Behaviors::same())
      })
    }
  });
  let ack_actor = system.as_untyped().spawn(ack_props.to_untyped()).expect("spawn ack receiver");
  let ack_ref = TypedActorRef::<crate::core::typed::receptionist::Registered>::from_untyped(ack_actor.into_actor_ref());

  let routee_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.into_actor_ref());

  receptionist.tell(Receptionist::register_with_ack(&key, routee_ref, ack_ref));
  wait_until(|| captured_registered.lock().is_some());

  // When/Then: is_for_key returns true for matching key
  let registered = captured_registered.lock().clone().expect("registered ack");
  assert!(registered.is_for_key(&key));

  // And: is_for_key returns false for a different key
  let other_key = ServiceKey::<u32>::new("other-svc");
  assert!(!registered.is_for_key(&other_key));

  system.terminate().expect("terminate");
}

#[test]
fn register_without_ack_still_works() {
  // Given: a system with a receptionist
  let system = new_test_system();
  let mut receptionist = system.receptionist_ref().expect("system receptionist");
  let key = ServiceKey::<u32>::new("no-ack-svc");

  let routee_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.into_actor_ref());

  // When: existing register() (without ack) is used
  receptionist.tell(Receptionist::register(&key, routee_ref));

  // Then: the actor is still registered and findable
  wait_until(|| !find_listing(&mut receptionist, &key).is_empty());

  let listing = find_listing(&mut receptionist, &key);
  assert_eq!(listing.refs().len(), 1);

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
  let subscriber_ref = TypedActorRef::<Listing>::from_untyped(subscriber.clone().into_actor_ref());

  receptionist.tell(Receptionist::subscribe(&key, subscriber_ref));
  wait_until(|| *updates.lock() == 1);

  subscriber.stop().expect("stop listing subscriber");

  let routee_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.into_actor_ref());
  receptionist.tell(Receptionist::register(&key, routee_ref));

  for _ in 0..10_000 {
    assert_eq!(*updates.lock(), 1);
    spin_loop();
  }

  system.terminate().expect("terminate");
}
