use alloc::{string::String, vec::Vec};
use core::hint::spin_loop;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex, shared::Shared};

use super::{ReceptionistExtensionId, handle_command};
use crate::core::{
  kernel::{
    actor::{
      Pid,
      actor_ref::{ActorRef, ActorRefSender, NullSender, SendOutcome},
      error::{ActorError, SendError},
      messaging::AnyMessage,
      scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
    },
    event::{
      logging::LogLevel,
      stream::{EventStreamEvent, EventStreamSubscriber, EventStreamSubscription, subscriber_handle},
    },
    system::ActorSystem,
  },
  typed::{
    TypedActorRef, TypedActorSystem, TypedProps,
    dsl::Behaviors,
    receptionist::{Deregistered, Listing, Receptionist, ReceptionistCommand, Registered, ServiceKey},
  },
};

struct RecordingSubscriber {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new(events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

struct ListingSender {
  listings: ArcShared<SpinSyncMutex<Vec<Listing>>>,
}

impl ListingSender {
  fn new(listings: ArcShared<SpinSyncMutex<Vec<Listing>>>) -> Self {
    Self { listings }
  }
}

impl ActorRefSender for ListingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    let Some(listing) = message.payload().downcast_ref::<Listing>() else {
      return Err(SendError::invalid_payload(message, "expected Listing"));
    };
    self.listings.lock().push(listing.clone());
    Ok(SendOutcome::Delivered)
  }
}

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
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system")
}

fn subscribe_log_recorder(
  system: &ActorSystem,
) -> (ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>, EventStreamSubscription) {
  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let subscription = system.event_stream().subscribe(&subscriber);
  (events, subscription)
}

fn has_warn_log(events: &ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>, needle: &str) -> bool {
  events.lock().iter().any(|event| {
    matches!(event, EventStreamEvent::Log(log) if log.level() == LogLevel::Warn && log.message().contains(needle))
  })
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
  let mut receptionist = system.receptionist();
  let key = ServiceKey::<u32>::new("svc");

  let updates = ArcShared::new(SpinSyncMutex::new(0_usize));
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
  let mut receptionist = system.receptionist();
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
  let mut receptionist = system.receptionist();
  let key = ServiceKey::<u32>::new("ack-svc");

  let ack_received = ArcShared::new(SpinSyncMutex::new(false));
  let ack_service_id = ArcShared::new(SpinSyncMutex::new(String::new()));

  // Spawn a receiver for Registered ack
  let ack_props = TypedProps::<Registered>::from_behavior_factory({
    let ack_received = ack_received.clone();
    let ack_service_id = ack_service_id.clone();
    move || {
      let ack_received = ack_received.clone();
      let ack_service_id = ack_service_id.clone();
      Behaviors::receive_message(move |_ctx, registered: &Registered| {
        *ack_service_id.lock() = String::from(registered.service_id());
        *ack_received.lock() = true;
        Ok(Behaviors::same())
      })
    }
  });
  let ack_actor = system.as_untyped().spawn(ack_props.to_untyped()).expect("spawn ack receiver");
  let ack_ref = TypedActorRef::<Registered>::from_untyped(ack_actor.into_actor_ref());

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
  let mut receptionist = system.receptionist();
  let key = ServiceKey::<u32>::new("dereg-svc");

  let routee_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.into_actor_ref());

  // First register the routee
  receptionist.tell(Receptionist::register(&key, routee_ref.clone()));
  wait_until(|| !find_listing(&mut receptionist, &key).is_empty());

  // Spawn an ack receiver for Deregistered
  let ack_received = ArcShared::new(SpinSyncMutex::new(false));
  let ack_service_id = ArcShared::new(SpinSyncMutex::new(String::new()));

  let ack_props = TypedProps::<Deregistered>::from_behavior_factory({
    let ack_received = ack_received.clone();
    let ack_service_id = ack_service_id.clone();
    move || {
      let ack_received = ack_received.clone();
      let ack_service_id = ack_service_id.clone();
      Behaviors::receive_message(move |_ctx, deregistered: &Deregistered| {
        *ack_service_id.lock() = String::from(deregistered.service_id());
        *ack_received.lock() = true;
        Ok(Behaviors::same())
      })
    }
  });
  let ack_actor = system.as_untyped().spawn(ack_props.to_untyped()).expect("spawn ack receiver");
  let ack_ref = TypedActorRef::<Deregistered>::from_untyped(ack_actor.into_actor_ref());

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
  let mut receptionist = system.receptionist();
  let key = ServiceKey::<u32>::new("key-check-svc");

  let captured_registered = ArcShared::new(SpinSyncMutex::new(Option::<Registered>::None));

  let ack_props = TypedProps::<Registered>::from_behavior_factory({
    let captured = captured_registered.clone();
    move || {
      let captured = captured.clone();
      Behaviors::receive_message(move |_ctx, registered: &Registered| {
        *captured.lock() = Some(registered.clone());
        Ok(Behaviors::same())
      })
    }
  });
  let ack_actor = system.as_untyped().spawn(ack_props.to_untyped()).expect("spawn ack receiver");
  let ack_ref = TypedActorRef::<Registered>::from_untyped(ack_actor.into_actor_ref());

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
  let mut receptionist = system.receptionist();
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
fn register_returns_error_and_does_not_store_registration_when_watch_fails() {
  let system = ActorSystem::new_empty();
  let state = Receptionist::empty_state();
  let key = ServiceKey::<u32>::new("watch-fail-register");
  let routee = ActorRef::new(Pid::new(701, 0), NullSender);
  let command = Receptionist::register(&key, TypedActorRef::from_untyped(routee.clone()));

  state.with_lock(|guard| {
    let error = handle_command(guard, &system, None, &command, |_| Err(ActorError::fatal("watch failed")))
      .expect_err("watch failure should abort registration");
    let registry_key = (String::from(key.id()), key.type_id());
    assert_eq!(format!("{error:?}"), format!("{:?}", ActorError::fatal("watch failed")));
    assert!(!guard.registrations.contains_key(&registry_key));
  });
}

#[test]
fn subscribe_returns_error_and_does_not_store_subscriber_when_watch_fails() {
  let system = ActorSystem::new_empty();
  let state = Receptionist::empty_state();
  let key = ServiceKey::<u32>::new("watch-fail-subscriber");
  let listings = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber =
    TypedActorRef::<Listing>::from_untyped(ActorRef::new(Pid::new(702, 0), ListingSender::new(listings.clone())));
  let command = Receptionist::subscribe(&key, subscriber.clone());

  state.with_lock(|guard| {
    let error = handle_command(guard, &system, None, &command, |_| Err(ActorError::fatal("watch failed")))
      .expect_err("watch failure should abort subscription");
    let registry_key = (String::from(key.id()), key.type_id());
    assert_eq!(format!("{error:?}"), format!("{:?}", ActorError::fatal("watch failed")));
    assert!(!guard.subscribers.contains_key(&registry_key));
  });

  assert_eq!(listings.lock().len(), 1, "initial listing should still be delivered");
}

#[test]
fn subscribe_logs_warn_and_preserves_subscriber_when_initial_listing_delivery_fails() {
  let system = ActorSystem::new_empty();
  let (events, _subscription) = subscribe_log_recorder(&system);
  let state = Receptionist::empty_state();
  let key = ServiceKey::<u32>::new("closed-subscriber");
  let subscriber = TypedActorRef::<Listing>::from_untyped(ActorRef::null());
  let command = Receptionist::subscribe(&key, subscriber.clone());

  state.with_lock(|guard| {
    handle_command(guard, &system, None, &command, |_| Ok(())).expect("subscribe should succeed");
    let registry_key = (String::from(key.id()), key.type_id());
    assert_eq!(guard.subscribers.get(&registry_key).map(Vec::len), Some(1));
    assert_eq!(guard.subscribers[&registry_key][0].pid(), subscriber.pid());
  });

  assert!(has_warn_log(&events, "receptionist failed to send initial listing to subscriber"));
  assert!(has_warn_log(&events, "service_id=closed-subscriber"));
}

#[test]
fn register_logs_warn_and_preserves_registration_when_notifying_closed_subscriber_fails() {
  let system = ActorSystem::new_empty();
  let (events, _subscription) = subscribe_log_recorder(&system);
  let state = Receptionist::empty_state();
  let key = ServiceKey::<u32>::new("notify-fail");
  let subscriber = TypedActorRef::<Listing>::from_untyped(ActorRef::null());
  let subscribe = Receptionist::subscribe(&key, subscriber);
  let routee = ActorRef::new(Pid::new(705, 0), NullSender);
  let register = Receptionist::register(&key, TypedActorRef::from_untyped(routee.clone()));

  state.with_lock(|guard| {
    handle_command(guard, &system, None, &subscribe, |_| Ok(())).expect("seed subscriber");
    handle_command(guard, &system, None, &register, |_| Ok(())).expect("register should stay best-effort");
    let registry_key = (String::from(key.id()), key.type_id());
    assert_eq!(guard.registrations.get(&registry_key).map(Vec::len), Some(1));
    assert_eq!(guard.registrations[&registry_key][0].pid(), routee.pid());
  });

  assert!(has_warn_log(&events, "receptionist failed to notify subscriber"));
  assert!(has_warn_log(&events, "service_id=notify-fail"));
}

#[test]
fn register_with_ack_logs_warn_and_preserves_registration_when_reply_target_is_closed() {
  let system = ActorSystem::new_empty();
  let (events, _subscription) = subscribe_log_recorder(&system);
  let state = Receptionist::empty_state();
  let key = ServiceKey::<u32>::new("ack-fail-register");
  let routee = ActorRef::new(Pid::new(703, 0), NullSender);
  let reply_to = TypedActorRef::from_untyped(ActorRef::null());
  let command = Receptionist::register_with_ack(&key, TypedActorRef::from_untyped(routee.clone()), reply_to);

  state.with_lock(|guard| {
    handle_command(guard, &system, None, &command, |_| Ok(())).expect("register_with_ack should still succeed");
    let registry_key = (String::from(key.id()), key.type_id());
    assert_eq!(guard.registrations.get(&registry_key).map(Vec::len), Some(1));
    assert_eq!(guard.registrations[&registry_key][0].pid(), routee.pid());
  });

  assert!(has_warn_log(&events, "receptionist failed to send Registered ack"));
}

#[test]
fn deregister_with_ack_logs_warn_and_removes_registration_when_reply_target_is_closed() {
  let system = ActorSystem::new_empty();
  let (events, _subscription) = subscribe_log_recorder(&system);
  let state = Receptionist::empty_state();
  let key = ServiceKey::<u32>::new("ack-fail-deregister");
  let routee = ActorRef::new(Pid::new(704, 0), NullSender);

  state.with_lock(|guard| {
    let register = Receptionist::register(&key, TypedActorRef::from_untyped(routee.clone()));
    handle_command(guard, &system, None, &register, |_| Ok(())).expect("seed registration");
  });

  let reply_to = TypedActorRef::from_untyped(ActorRef::null());
  let deregister = Receptionist::deregister_with_ack(&key, TypedActorRef::from_untyped(routee), reply_to);

  state.with_lock(|guard| {
    handle_command(guard, &system, None, &deregister, |_| Ok(())).expect("deregister_with_ack should still succeed");
    let registry_key = (String::from(key.id()), key.type_id());
    assert!(guard.registrations.get(&registry_key).is_none());
  });

  assert!(has_warn_log(&events, "receptionist failed to send Deregistered ack"));
}

#[test]
fn find_logs_warn_and_returns_ok_when_reply_target_is_closed() {
  let system = ActorSystem::new_empty();
  let (events, _subscription) = subscribe_log_recorder(&system);
  let state = Receptionist::empty_state();
  let key = ServiceKey::<u32>::new("find-closed-reply");
  let command = Receptionist::find(&key, TypedActorRef::from_untyped(ActorRef::null()));

  state.with_lock(|guard| {
    handle_command(guard, &system, None, &command, |_| Ok(())).expect("find should stay best-effort");
  });

  assert!(has_warn_log(&events, "receptionist failed to reply with listing"));
  assert!(has_warn_log(&events, "service_id=find-closed-reply"));
}

#[test]
fn terminated_subscriber_should_be_cleaned_up() {
  let system = new_test_system();
  let mut receptionist = system.receptionist();
  let key = ServiceKey::<u32>::new("svc");

  let updates = ArcShared::new(SpinSyncMutex::new(0_usize));
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

#[test]
fn get_returns_facade_whose_ref_matches_system_receptionist_ref() {
  let system = new_test_system();
  let expected = system.receptionist_ref().expect("system receptionist");
  let extension = Receptionist::get(&system);
  let actual = extension.r#ref();

  assert_eq!(actual, expected);

  system.terminate().expect("terminate");
}

#[test]
fn create_extension_returns_facade_whose_ref_matches_system_receptionist_ref() {
  let system = new_test_system();
  let expected = system.receptionist_ref().expect("system receptionist");
  let extension = Receptionist::create_extension(&system);
  let actual = extension.r#ref();

  assert_eq!(actual, expected);

  system.terminate().expect("terminate");
}

#[test]
fn get_registers_receptionist_extension_even_when_bootstrap_actor_is_missing() {
  let system = TypedActorSystem::<u32>::from_untyped(ActorSystem::new_empty());
  let extension_id = ReceptionistExtensionId::new();

  assert!(system.extension(&extension_id).is_none());

  let first = Receptionist::get(&system);
  let second = Receptionist::get(&system);
  let registered = system.extension(&extension_id).expect("registered receptionist extension");

  assert_eq!(first.r#ref().pid(), second.r#ref().pid());
  assert_eq!(first.r#ref().pid(), registered.with_ref(|receptionist| receptionist.r#ref().pid()));
  assert!(system.receptionist_ref().is_none());
}

#[test]
fn create_extension_supports_register_and_find_without_bootstrap_receptionist() {
  let system = TypedActorSystem::<u32>::from_untyped(ActorSystem::new_empty());
  let mut receptionist = Receptionist::create_extension(&system).r#ref();
  let key = ServiceKey::<u32>::new("standalone");
  let service_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let service = system.as_untyped().spawn_detached(service_props.to_untyped()).expect("spawn detached service");
  let service = TypedActorRef::<u32>::from_untyped(service.into_actor_ref());

  receptionist.tell(Receptionist::register(&key, service.clone()));

  let listing = find_listing(&mut receptionist, &key);

  assert_eq!(listing.refs().len(), 1);
  assert_eq!(listing.refs()[0].pid(), service.pid());
}

#[test]
fn get_cleans_up_terminated_routee_without_bootstrap_receptionist() {
  let system = TypedActorSystem::<u32>::from_untyped(ActorSystem::new_empty());
  let mut receptionist = Receptionist::get(&system).r#ref();
  let key = ServiceKey::<u32>::new("standalone-routee-cleanup");

  let routee_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let routee = system.as_untyped().spawn_detached(routee_props.to_untyped()).expect("spawn detached routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.clone().into_actor_ref());

  receptionist.tell(Receptionist::register(&key, routee_ref));
  wait_until(|| !find_listing(&mut receptionist, &key).is_empty());

  routee.stop().expect("stop detached routee");
  wait_until(|| find_listing(&mut receptionist, &key).is_empty());
}

#[test]
fn get_cleans_up_terminated_subscriber_without_bootstrap_receptionist() {
  let system = TypedActorSystem::<u32>::from_untyped(ActorSystem::new_empty());
  let mut receptionist = Receptionist::get(&system).r#ref();
  let key = ServiceKey::<u32>::new("standalone-subscriber-cleanup");

  let updates = ArcShared::new(SpinSyncMutex::new(0_usize));
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
  let subscriber =
    system.as_untyped().spawn_detached(subscriber_props.to_untyped()).expect("spawn detached subscriber");
  let subscriber_ref = TypedActorRef::<Listing>::from_untyped(subscriber.clone().into_actor_ref());

  receptionist.tell(Receptionist::subscribe(&key, subscriber_ref));
  wait_until(|| *updates.lock() == 1);

  subscriber.stop().expect("stop detached subscriber");

  let routee_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let routee = system.as_untyped().spawn_detached(routee_props.to_untyped()).expect("spawn detached routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.into_actor_ref());
  receptionist.tell(Receptionist::register(&key, routee_ref));

  for _ in 0..10_000 {
    assert_eq!(*updates.lock(), 1);
    spin_loop();
  }
}
