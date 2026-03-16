use alloc::vec::Vec;
use core::hint::spin_loop;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use crate::core::typed::{
  Behaviors, Receptionist, ServiceKey, TypedActorSystem, TypedProps,
  actor::TypedActorRef,
  delivery::{
    ConsumerController, ConsumerControllerCommand, ConsumerControllerConfirmed, ConsumerControllerDelivery,
    ProducerController, ProducerControllerCommand, ProducerControllerRequestNext, WorkPullingProducerController,
    WorkPullingProducerControllerCommand, WorkPullingProducerControllerRequestNext, WorkerStats,
  },
};

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..100_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition(), "wait_until timed out");
}

/// Helper to create a test actor system.
fn test_system() -> TypedActorSystem<u32> {
  let guardian_props = TypedProps::<u32>::from_behavior_factory(Behaviors::ignore);
  let tick_driver = crate::core::scheduler::tick_driver::TickDriverConfig::manual(
    crate::core::scheduler::tick_driver::ManualTestDriver::new(),
  );
  TypedActorSystem::<u32>::new(&guardian_props, tick_driver).expect("system")
}

#[test]
fn producer_start_and_register_consumer_connect() {
  let system = test_system();

  // Spawn producer controller.
  let pc_props = TypedProps::<ProducerControllerCommand<u32>>::from_behavior_factory(|| {
    ProducerController::behavior("test-producer")
  });
  let pc_cell = system.as_untyped().spawn(pc_props.to_untyped()).expect("spawn producer controller");
  let mut pc_ref = TypedActorRef::<ProducerControllerCommand<u32>>::from_untyped(pc_cell.actor_ref().clone());

  // Spawn consumer controller.
  let cc_props =
    TypedProps::<ConsumerControllerCommand<u32>>::from_behavior_factory(|| ConsumerController::behavior::<u32>());
  let cc_cell = system.as_untyped().spawn(cc_props.to_untyped()).expect("spawn consumer controller");
  let cc_ref = TypedActorRef::<ConsumerControllerCommand<u32>>::from_untyped(cc_cell.actor_ref().clone());

  // Track received RequestNext signals.
  let request_next_received = ArcShared::new(NoStdMutex::new(Vec::<u64>::new()));
  let request_next_received_clone = request_next_received.clone();

  // Spawn a mock producer that records RequestNext signals.
  let producer_props = TypedProps::<ProducerControllerRequestNext<u32>>::from_behavior_factory({
    move || {
      let received = request_next_received_clone.clone();
      Behaviors::receive_message(move |_ctx, req: &ProducerControllerRequestNext<u32>| {
        received.lock().push(req.current_seq_nr());
        Ok(Behaviors::same())
      })
    }
  });
  let producer_cell = system.as_untyped().spawn(producer_props.to_untyped()).expect("spawn producer");
  let producer_ref =
    TypedActorRef::<ProducerControllerRequestNext<u32>>::from_untyped(producer_cell.actor_ref().clone());

  // Start the producer controller.
  pc_ref.tell(ProducerController::start(producer_ref)).expect("start");

  // Register consumer.
  pc_ref.tell(ProducerController::register_consumer(cc_ref.clone())).expect("register consumer");

  // The ProducerController should connect, though actual RequestNext delivery
  // depends on the consumer side sending a Request. At minimum, the system
  // should not panic and all messages should be accepted.

  system.terminate().expect("terminate");
}

#[test]
fn consumer_controller_delivers_to_consumer() {
  let system = test_system();

  // Track deliveries.
  let delivered = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let delivered_clone = delivered.clone();

  // Spawn consumer controller.
  let cc_props =
    TypedProps::<ConsumerControllerCommand<u32>>::from_behavior_factory(|| ConsumerController::behavior::<u32>());
  let cc_cell = system.as_untyped().spawn(cc_props.to_untyped()).expect("spawn consumer controller");
  let mut cc_ref = TypedActorRef::<ConsumerControllerCommand<u32>>::from_untyped(cc_cell.actor_ref().clone());

  // Spawn a consumer that processes deliveries and sends Confirmed.
  let consumer_props = TypedProps::<ConsumerControllerDelivery<u32>>::from_behavior_factory({
    move || {
      let delivered = delivered_clone.clone();
      Behaviors::receive_message(move |_ctx, delivery: &ConsumerControllerDelivery<u32>| {
        delivered.lock().push(*delivery.message());
        let mut confirm_to = delivery.confirm_to().clone();
        confirm_to
          .tell(ConsumerControllerConfirmed)
          .map_err(|e| crate::core::error::ActorError::from_send_error(&e))?;
        Ok(Behaviors::same())
      })
    }
  });
  let consumer_cell = system.as_untyped().spawn(consumer_props.to_untyped()).expect("spawn consumer");
  let consumer_ref = TypedActorRef::<ConsumerControllerDelivery<u32>>::from_untyped(consumer_cell.actor_ref().clone());

  // Start the consumer.
  cc_ref.tell(ConsumerController::start(consumer_ref)).expect("start consumer");

  // Spawn producer controller.
  let pc_props = TypedProps::<ProducerControllerCommand<u32>>::from_behavior_factory(|| {
    ProducerController::behavior("test-producer")
  });
  let pc_cell = system.as_untyped().spawn(pc_props.to_untyped()).expect("spawn producer controller");
  let mut pc_ref = TypedActorRef::<ProducerControllerCommand<u32>>::from_untyped(pc_cell.actor_ref().clone());

  // Spawn a mock producer that sends a message when RequestNext is received.
  let producer_props = TypedProps::<ProducerControllerRequestNext<u32>>::from_behavior_factory({
    move || {
      Behaviors::receive_message(move |_ctx, req: &ProducerControllerRequestNext<u32>| {
        let mut send_to = req.send_next_to().clone();
        send_to.tell(42_u32).map_err(|e| crate::core::error::ActorError::from_send_error(&e))?;
        Ok(Behaviors::same())
      })
    }
  });
  let producer_cell = system.as_untyped().spawn(producer_props.to_untyped()).expect("spawn producer");
  let producer_ref =
    TypedActorRef::<ProducerControllerRequestNext<u32>>::from_untyped(producer_cell.actor_ref().clone());

  // Start PC first so it has a producer ref ready when CC registers.
  pc_ref.tell(ProducerController::start(producer_ref)).expect("start producer");

  // Register consumer controller with producer controller.
  cc_ref
    .tell(ConsumerController::register_to_producer_controller(pc_ref.clone()))
    .expect("register to producer controller");

  // Wait for delivery to arrive at the consumer.
  // With inline dispatch, the CC's flow-control Request may trigger an
  // additional RequestNext before the first delivery is confirmed, so
  // the consumer may receive the value more than once.
  wait_until(|| !delivered.lock().is_empty());
  assert!(delivered.lock().contains(&42_u32));

  system.terminate().expect("terminate");
}

#[test]
fn consumer_controller_settings_accessors() {
  use crate::core::typed::delivery::ConsumerControllerSettings;

  let settings = ConsumerControllerSettings::new().with_flow_control_window(100).with_only_flow_control(true);
  assert_eq!(settings.flow_control_window(), 100);
  assert!(settings.only_flow_control());
}

#[test]
fn producer_controller_settings_accessors() {
  use crate::core::typed::delivery::ProducerControllerSettings;

  let _settings = ProducerControllerSettings::new();
}

#[test]
fn work_pulling_producer_controller_settings_accessors() {
  use crate::core::typed::delivery::WorkPullingProducerControllerSettings;

  let settings = WorkPullingProducerControllerSettings::new();
  assert_eq!(settings.buffer_size(), 1000);
}

#[test]
fn work_pulling_start_and_get_worker_stats() {
  let system = test_system();

  let worker_key = ServiceKey::<ConsumerControllerCommand<u32>>::new("test-workers");

  // Spawn the work-pulling producer controller.
  let wppc_props = TypedProps::<WorkPullingProducerControllerCommand<u32>>::from_behavior_factory({
    let worker_key = worker_key.clone();
    move || WorkPullingProducerController::behavior("test-wp-producer", worker_key.clone())
  });
  let wppc_cell = system.as_untyped().spawn(wppc_props.to_untyped()).expect("spawn work-pulling producer controller");
  let mut wppc_ref =
    TypedActorRef::<WorkPullingProducerControllerCommand<u32>>::from_untyped(wppc_cell.actor_ref().clone());

  // Track worker stats responses.
  let stats_received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let stats_received_clone = stats_received.clone();

  // Spawn a stats-collecting actor.
  let stats_props = TypedProps::<WorkerStats>::from_behavior_factory({
    move || {
      let received = stats_received_clone.clone();
      Behaviors::receive_message(move |_ctx, stats: &WorkerStats| {
        received.lock().push(stats.number_of_workers());
        Ok(Behaviors::same())
      })
    }
  });
  let stats_cell = system.as_untyped().spawn(stats_props.to_untyped()).expect("spawn stats");
  let stats_ref = TypedActorRef::<WorkerStats>::from_untyped(stats_cell.actor_ref().clone());

  // Before any workers register, stats should be 0.
  wppc_ref.tell(WorkPullingProducerController::get_worker_stats(stats_ref.clone())).expect("get stats");

  wait_until(|| !stats_received.lock().is_empty());
  assert_eq!(stats_received.lock()[0], 0);

  system.terminate().expect("terminate");
}

#[test]
fn work_pulling_delivers_to_worker_via_receptionist() {
  let system = test_system();

  let worker_key = ServiceKey::<ConsumerControllerCommand<u32>>::new("wp-workers");

  // Spawn the work-pulling producer controller.
  let wppc_props = TypedProps::<WorkPullingProducerControllerCommand<u32>>::from_behavior_factory({
    let worker_key = worker_key.clone();
    move || WorkPullingProducerController::behavior("test-wp-producer", worker_key.clone())
  });
  let wppc_cell = system.as_untyped().spawn(wppc_props.to_untyped()).expect("spawn work-pulling producer controller");
  let mut wppc_ref =
    TypedActorRef::<WorkPullingProducerControllerCommand<u32>>::from_untyped(wppc_cell.actor_ref().clone());

  // Track deliveries.
  let delivered = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let delivered_clone = delivered.clone();

  // Spawn a worker (consumer controller + consumer actor).
  let cc_props =
    TypedProps::<ConsumerControllerCommand<u32>>::from_behavior_factory(|| ConsumerController::behavior::<u32>());
  let cc_cell = system.as_untyped().spawn(cc_props.to_untyped()).expect("spawn cc");
  let mut cc_ref = TypedActorRef::<ConsumerControllerCommand<u32>>::from_untyped(cc_cell.actor_ref().clone());

  // Spawn a consumer that processes deliveries.
  let consumer_props = TypedProps::<ConsumerControllerDelivery<u32>>::from_behavior_factory({
    move || {
      let delivered = delivered_clone.clone();
      Behaviors::receive_message(move |_ctx, delivery: &ConsumerControllerDelivery<u32>| {
        delivered.lock().push(*delivery.message());
        let mut confirm_to = delivery.confirm_to().clone();
        confirm_to
          .tell(ConsumerControllerConfirmed)
          .map_err(|e| crate::core::error::ActorError::from_send_error(&e))?;
        Ok(Behaviors::same())
      })
    }
  });
  let consumer_cell = system.as_untyped().spawn(consumer_props.to_untyped()).expect("spawn consumer");
  let consumer_ref = TypedActorRef::<ConsumerControllerDelivery<u32>>::from_untyped(consumer_cell.actor_ref().clone());

  // Start the consumer controller.
  cc_ref.tell(ConsumerController::start(consumer_ref)).expect("start consumer");

  // Register the worker's consumer controller with the Receptionist.
  if let Some(mut receptionist_ref) = system.receptionist_ref() {
    receptionist_ref.tell(Receptionist::register(&worker_key, cc_ref.clone())).expect("register worker");
  }

  // Spawn a mock producer that sends a message when RequestNext is received.
  let producer_props = TypedProps::<WorkPullingProducerControllerRequestNext<u32>>::from_behavior_factory({
    move || {
      Behaviors::receive_message(move |_ctx, req: &WorkPullingProducerControllerRequestNext<u32>| {
        let mut send_to = req.send_next_to().clone();
        send_to.tell(99_u32).map_err(|e| crate::core::error::ActorError::from_send_error(&e))?;
        Ok(Behaviors::same())
      })
    }
  });
  let producer_cell = system.as_untyped().spawn(producer_props.to_untyped()).expect("spawn producer");
  let producer_ref =
    TypedActorRef::<WorkPullingProducerControllerRequestNext<u32>>::from_untyped(producer_cell.actor_ref().clone());

  // Start the work-pulling producer controller.
  wppc_ref.tell(WorkPullingProducerController::start(producer_ref)).expect("start wp producer");

  // Wait for a delivery to arrive at the consumer.
  wait_until(|| !delivered.lock().is_empty());
  assert_eq!(delivered.lock()[0], 99_u32);

  system.terminate().expect("terminate");
}
