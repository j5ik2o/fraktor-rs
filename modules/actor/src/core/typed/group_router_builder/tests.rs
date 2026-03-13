use alloc::vec::Vec;
use core::{any::TypeId, hint::spin_loop};

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::typed::{
  Behaviors, Listing, Receptionist, ReceptionistCommand, ServiceKey, TypedActorSystem, TypedProps,
  actor::TypedActorRef, group_router_builder::GroupRouterBuilder, routers::Routers,
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
fn should_create_builder_from_service_key() {
  let key = ServiceKey::<u32>::new("test-group");
  let _builder = GroupRouterBuilder::new(key);
}

#[test]
fn group_router_builder_with_random_routing_builds_behavior() {
  let key = ServiceKey::<u32>::new("test-group-random-build");
  let _behavior = Routers::group(key).with_random_routing(7).build();
}

#[test]
fn group_router_builder_with_round_robin_routing_builds_behavior() {
  let key = ServiceKey::<u32>::new("test-group-round-robin-build");
  let _behavior = Routers::group(key).with_round_robin_routing().build();
}

#[test]
fn group_router_builder_with_consistent_hash_routing_builds_behavior() {
  let key = ServiceKey::<u32>::new("test-group-consistent-hash-build");
  let _behavior = Routers::group(key).with_consistent_hash_routing(|message| message.to_string()).build();
}

#[test]
fn group_router_should_route_via_system_receptionist() {
  let key = ServiceKey::<u32>::new("test-group");
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let router_props = TypedProps::<u32>::from_behavior_factory({
    let key = key.clone();
    move || Routers::group(key.clone()).build()
  });
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system");
  let router = system.as_untyped().spawn(router_props.to_untyped()).expect("spawn group router");
  let mut router = TypedActorRef::<u32>::from_untyped(router.actor_ref().clone());
  let mut receptionist = system.receptionist_ref().expect("system receptionist");

  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routee_props = TypedProps::<u32>::from_behavior_factory({
    let records = records.clone();
    move || {
      let records = records.clone();
      Behaviors::receive_message(move |_ctx, message: &u32| {
        records.lock().push(*message);
        Ok(Behaviors::same())
      })
    }
  });
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.actor_ref().clone());
  receptionist.tell(Receptionist::register(&key, routee_ref)).expect("register routee to receptionist");

  router.tell(42_u32).expect("route message");
  wait_until(|| records.lock().as_slice() == [42_u32]);

  system.terminate().expect("terminate");
}

#[test]
fn group_router_with_consistent_hash_routes_same_message_to_same_routee() {
  let key = ServiceKey::<u32>::new("test-group-consistent-hash");
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system");

  let router_props = TypedProps::<u32>::from_behavior_factory({
    let key = key.clone();
    move || Routers::group(key.clone()).with_consistent_hash_routing(|message| message.to_string()).build()
  });
  let router = system.as_untyped().spawn(router_props.to_untyped()).expect("spawn group router");
  let mut router = TypedActorRef::<u32>::from_untyped(router.actor_ref().clone());
  let mut receptionist = system.receptionist_ref().expect("system receptionist");

  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  for routee_index in 0..2_usize {
    let routee_props = TypedProps::<u32>::from_behavior_factory({
      let records = records.clone();
      move || {
        let records = records.clone();
        Behaviors::receive_message(move |_ctx, message: &u32| {
          records.lock().push((routee_index, *message));
          Ok(Behaviors::same())
        })
      }
    });
    let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
    let routee_ref = TypedActorRef::<u32>::from_untyped(routee.actor_ref().clone());
    receptionist.tell(Receptionist::register(&key, routee_ref)).expect("register routee");
  }

  wait_until(|| {
    let _ = router.tell(5_u32);
    records.lock().iter().any(|(_, message)| *message == 5)
  });
  router.tell(5_u32).expect("route same message");
  wait_until(|| records.lock().iter().filter(|(_, message)| *message == 5).count() == 2);

  let routed_indices: Vec<usize> =
    records.lock().iter().filter_map(|(routee_index, message)| (*message == 5).then_some(*routee_index)).collect();
  assert_eq!(routed_indices.len(), 2);
  assert_eq!(routed_indices[0], routed_indices[1]);

  system.terminate().expect("terminate");
}

#[test]
fn group_router_with_round_robin_routes_across_routees_in_order() {
  let key = ServiceKey::<u32>::new("test-group-round-robin-routing");
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system");

  let router_props = TypedProps::<u32>::from_behavior_factory({
    let key = key.clone();
    move || Routers::group(key.clone()).with_round_robin_routing().build()
  });
  let router = system.as_untyped().spawn(router_props.to_untyped()).expect("spawn group router");
  let mut router = TypedActorRef::<u32>::from_untyped(router.actor_ref().clone());
  let mut receptionist = system.receptionist_ref().expect("system receptionist");

  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  for routee_index in 0..2_usize {
    let routee_props = TypedProps::<u32>::from_behavior_factory({
      let records = records.clone();
      move || {
        let records = records.clone();
        Behaviors::receive_message(move |_ctx, message: &u32| {
          records.lock().push((routee_index, *message));
          Ok(Behaviors::same())
        })
      }
    });
    let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
    let routee_ref = TypedActorRef::<u32>::from_untyped(routee.actor_ref().clone());
    receptionist.tell(Receptionist::register(&key, routee_ref)).expect("register routee");
  }

  wait_until(|| {
    let _ = router.tell(100_u32);
    !records.lock().is_empty()
  });
  records.lock().clear();

  for message in 0..4_u32 {
    router.tell(message).expect("tell");
  }
  wait_until(|| records.lock().len() == 4);

  let mut routee_by_message = [usize::MAX; 4];
  for (routee_index, message) in records.lock().iter().copied() {
    routee_by_message[message as usize] = routee_index;
  }
  assert_ne!(routee_by_message[0], routee_by_message[1]);
  assert_eq!(routee_by_message[0], routee_by_message[2]);
  assert_eq!(routee_by_message[1], routee_by_message[3]);

  system.terminate().expect("terminate");
}

#[test]
fn group_router_with_random_routing_uses_random_selector_branch() {
  let key = ServiceKey::<u32>::new("test-group-random-routing");
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system");

  let router_props = TypedProps::<u32>::from_behavior_factory({
    let key = key.clone();
    move || Routers::group(key.clone()).with_random_routing(11).build()
  });
  let router = system.as_untyped().spawn(router_props.to_untyped()).expect("spawn group router");
  let mut router = TypedActorRef::<u32>::from_untyped(router.actor_ref().clone());
  let mut receptionist = system.receptionist_ref().expect("system receptionist");

  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  for routee_index in 0..2_usize {
    let routee_props = TypedProps::<u32>::from_behavior_factory({
      let records = records.clone();
      move || {
        let records = records.clone();
        Behaviors::receive_message(move |_ctx, message: &u32| {
          records.lock().push((routee_index, *message));
          Ok(Behaviors::same())
        })
      }
    });
    let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
    let routee_ref = TypedActorRef::<u32>::from_untyped(routee.actor_ref().clone());
    receptionist.tell(Receptionist::register(&key, routee_ref)).expect("register routee");
  }

  wait_until(|| {
    let _ = router.tell(200_u32);
    !records.lock().is_empty()
  });
  records.lock().clear();

  for message in 0..6_u32 {
    router.tell(message).expect("tell");
  }
  wait_until(|| records.lock().len() == 6);

  // 実装ヘルパーではなく、観測可能な振る舞いとして有効な routee へ
  // 分配され、固定 seed でも片寄り切らないことを確認する。
  let mut routee_by_message = [usize::MAX; 6];
  for (routee_index, message) in records.lock().iter().copied() {
    routee_by_message[message as usize] = routee_index;
  }
  assert!(routee_by_message.iter().all(|index| *index < 2));
  assert!(routee_by_message.contains(&0));
  assert!(routee_by_message.contains(&1));

  system.terminate().expect("terminate");
}

#[test]
fn group_router_uses_round_robin_routing_by_default() {
  let key = ServiceKey::<u32>::new("test-group-default-round-robin-routing");
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system");

  let router_props = TypedProps::<u32>::from_behavior_factory({
    let key = key.clone();
    move || Routers::group(key.clone()).build()
  });
  let router = system.as_untyped().spawn(router_props.to_untyped()).expect("spawn group router");
  let mut router = TypedActorRef::<u32>::from_untyped(router.actor_ref().clone());
  let mut receptionist = system.receptionist_ref().expect("system receptionist");

  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  for routee_index in 0..2_usize {
    let routee_props = TypedProps::<u32>::from_behavior_factory({
      let records = records.clone();
      move || {
        let records = records.clone();
        Behaviors::receive_message(move |_ctx, message: &u32| {
          records.lock().push((routee_index, *message));
          Ok(Behaviors::same())
        })
      }
    });
    let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
    let routee_ref = TypedActorRef::<u32>::from_untyped(routee.actor_ref().clone());
    receptionist.tell(Receptionist::register(&key, routee_ref)).expect("register routee");
  }

  wait_until(|| {
    let _ = router.tell(300_u32);
    !records.lock().is_empty()
  });
  records.lock().clear();

  for message in 0..4_u32 {
    router.tell(message).expect("tell");
  }
  wait_until(|| records.lock().len() == 4);

  let mut routee_by_message = [usize::MAX; 4];
  for (routee_index, message) in records.lock().iter().copied() {
    routee_by_message[message as usize] = routee_index;
  }
  assert!(routee_by_message.iter().all(|index| *index < 2));
  assert_ne!(routee_by_message[0], routee_by_message[1]);
  assert_eq!(routee_by_message[0], routee_by_message[2]);
  assert_eq!(routee_by_message[1], routee_by_message[3]);

  system.terminate().expect("terminate");
}

#[test]
fn group_router_should_route_via_explicit_receptionist() {
  let key = ServiceKey::<u32>::new("test-group-explicit");
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system");
  let receptionist_props = TypedProps::<ReceptionistCommand>::from_behavior_factory(Receptionist::behavior);
  let receptionist = system.as_untyped().spawn(receptionist_props.to_untyped()).expect("spawn explicit receptionist");
  let receptionist_ref = TypedActorRef::<ReceptionistCommand>::from_untyped(receptionist.actor_ref().clone());

  let router_props = TypedProps::<u32>::from_behavior_factory({
    let key = key.clone();
    let receptionist_ref = receptionist_ref.clone();
    move || Routers::group(key.clone()).build_with_receptionist(receptionist_ref.clone())
  });
  let router = system.as_untyped().spawn(router_props.to_untyped()).expect("spawn group router");
  let mut router = TypedActorRef::<u32>::from_untyped(router.actor_ref().clone());
  let mut explicit_receptionist = receptionist_ref;

  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routee_props = TypedProps::<u32>::from_behavior_factory({
    let records = records.clone();
    move || {
      let records = records.clone();
      Behaviors::receive_message(move |_ctx, message: &u32| {
        records.lock().push(*message);
        Ok(Behaviors::same())
      })
    }
  });
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.actor_ref().clone());
  explicit_receptionist
    .tell(Receptionist::register(&key, routee_ref))
    .expect("register routee to explicit receptionist");

  wait_until(|| {
    let _ = router.tell(64_u32);
    records.lock().as_slice().contains(&64_u32)
  });

  system.terminate().expect("terminate");
}

#[test]
fn group_router_should_ignore_mismatched_listing_update() {
  let key = ServiceKey::<u32>::new("test-group-mismatch");
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system");

  let records = ArcShared::new(NoStdMutex::new(Vec::new()));
  let routee_props = TypedProps::<u32>::from_behavior_factory({
    let records = records.clone();
    move || {
      let records = records.clone();
      Behaviors::receive_message(move |_ctx, message: &u32| {
        records.lock().push(*message);
        Ok(Behaviors::same())
      })
    }
  });
  let routee = system.as_untyped().spawn(routee_props.to_untyped()).expect("spawn routee");
  let routee_ref = TypedActorRef::<u32>::from_untyped(routee.actor_ref().clone());

  let mismatched_records = ArcShared::new(NoStdMutex::new(Vec::new()));
  let mismatched_routee_props = TypedProps::<u64>::from_behavior_factory({
    let mismatched_records = mismatched_records.clone();
    move || {
      let mismatched_records = mismatched_records.clone();
      Behaviors::receive_message(move |_ctx, message: &u64| {
        mismatched_records.lock().push(*message);
        Ok(Behaviors::same())
      })
    }
  });
  let mismatched_routee =
    system.as_untyped().spawn(mismatched_routee_props.to_untyped()).expect("spawn mismatched routee");
  let mismatched_routee_ref = TypedActorRef::<u64>::from_untyped(mismatched_routee.actor_ref().clone());

  let subscriber = ArcShared::new(NoStdMutex::new(None::<TypedActorRef<Listing>>));
  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let receptionist_props = TypedProps::<ReceptionistCommand>::from_behavior_factory({
    let key = key.clone();
    let routee_ref = routee_ref.clone();
    let mismatched_routee_ref = mismatched_routee_ref.clone();
    let subscriber = subscriber.clone();
    let events = events.clone();
    move || {
      let key = key.clone();
      let routee_ref = routee_ref.clone();
      let mismatched_routee_ref = mismatched_routee_ref.clone();
      let subscriber = subscriber.clone();
      let events = events.clone();
      Behaviors::receive_message(move |_ctx, command: &ReceptionistCommand| {
        match command {
          | ReceptionistCommand::Subscribe { service_id, type_id, subscriber: reply_to }
            if service_id == key.id() && *type_id == key.type_id() =>
          {
            *subscriber.lock() = Some(reply_to.clone());
            events.lock().push("subscribed");

            let listing = Listing::new(service_id.clone(), *type_id, vec![routee_ref.clone().into_untyped()]);
            let mut reply_to = reply_to.clone();
            reply_to.tell(listing).expect("send initial listing");
          },
          | ReceptionistCommand::Register { .. } => {
            let reply_to = subscriber.lock().clone();
            if let Some(mut reply_to) = reply_to {
              events.lock().push("mismatch_sent");
              let listing =
                Listing::new(key.id(), TypeId::of::<u64>(), vec![mismatched_routee_ref.clone().into_untyped()]);
              reply_to.tell(listing).expect("send mismatched listing");
            }
          },
          | _ => {},
        }
        Ok(Behaviors::same())
      })
    }
  });
  let receptionist = system.as_untyped().spawn(receptionist_props.to_untyped()).expect("spawn explicit receptionist");
  let receptionist_ref = TypedActorRef::<ReceptionistCommand>::from_untyped(receptionist.actor_ref().clone());

  let router_props = TypedProps::<u32>::from_behavior_factory({
    let key = key.clone();
    let receptionist_ref = receptionist_ref.clone();
    move || Routers::group(key.clone()).build_with_receptionist(receptionist_ref.clone())
  });
  let router = system.as_untyped().spawn(router_props.to_untyped()).expect("spawn group router");
  let mut router = TypedActorRef::<u32>::from_untyped(router.actor_ref().clone());
  let mut explicit_receptionist = receptionist_ref;

  wait_until(|| {
    let _ = router.tell(1_u32);
    records.lock().contains(&1_u32)
  });

  explicit_receptionist.tell(Receptionist::register(&key, routee_ref)).expect("trigger mismatched listing update");
  wait_until(|| events.lock().contains(&"mismatch_sent"));

  for _ in 0..10_000 {
    spin_loop();
  }

  router.tell(2_u32).expect("route message after mismatched listing");
  wait_until(|| records.lock().contains(&2_u32));
  assert!(mismatched_records.lock().is_empty());

  system.terminate().expect("terminate");
}

#[test]
fn group_router_should_unsubscribe_when_stopped() {
  let key = ServiceKey::<u32>::new("test-group-unsubscribe");
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system");

  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let receptionist_props = TypedProps::<ReceptionistCommand>::from_behavior_factory({
    let events = events.clone();
    move || {
      let events = events.clone();
      Behaviors::receive_message(move |_ctx, command: &ReceptionistCommand| {
        let mut guard = events.lock();
        match command {
          | ReceptionistCommand::Subscribe { .. } => guard.push("subscribe"),
          | ReceptionistCommand::Unsubscribe { .. } => guard.push("unsubscribe"),
          | _ => {},
        }
        Ok(Behaviors::same())
      })
    }
  });
  let receptionist = system.as_untyped().spawn(receptionist_props.to_untyped()).expect("spawn tracking receptionist");
  let receptionist_ref = TypedActorRef::<ReceptionistCommand>::from_untyped(receptionist.actor_ref().clone());

  let router_props = TypedProps::<u32>::from_behavior_factory({
    let key = key.clone();
    let receptionist_ref = receptionist_ref.clone();
    move || Routers::group(key.clone()).build_with_receptionist(receptionist_ref.clone())
  });
  let router = system.as_untyped().spawn(router_props.to_untyped()).expect("spawn group router");

  wait_until(|| events.lock().contains(&"subscribe"));
  router.stop().expect("stop group router");
  wait_until(|| events.lock().contains(&"unsubscribe"));

  system.terminate().expect("terminate");
}
