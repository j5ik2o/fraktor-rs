use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::{any::TypeId, time::Duration};

use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorCell, ActorContext, Pid,
    actor_ref::{ActorRefSender, NullSender, SendOutcome},
    error::{ActorError, SendError},
    messaging::{AnyMessage, AnyMessageView, system_message::SystemMessage},
    props::Props,
  },
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedLock, SpinSyncMutex};

use super::*;
use crate::{
  TypedActorRef,
  actor::TypedActorContext,
  delivery::{
    ConsumerControllerCommand, DurableProducerQueueCommand, MessageSent, ProducerControllerCommand,
    ProducerControllerConfig, ProducerControllerRequestNext, StoreMessageSentAck, WorkPullingProducerControllerCommand,
    WorkPullingProducerControllerConfig, producer_controller_command::ProducerControllerCommandKind,
    work_pulling_producer_controller_command::WorkPullingProducerControllerCommandKind,
  },
  receptionist::{Listing, ServiceKey},
};

fn make_typed_ref<M: Send + Sync + 'static>() -> TypedActorRef<M> {
  TypedActorRef::from_untyped(crate::test_support::actor_ref_with_sender(Pid::new(1, 0), NullSender))
}

struct FailingSender;

impl ActorRefSender for FailingSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::closed(message))
  }
}

struct StopRecorderActor {
  lifecycle: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl StopRecorderActor {
  fn new(lifecycle: ArcShared<SpinSyncMutex<Vec<&'static str>>>) -> Self {
    Self { lifecycle }
  }
}

impl Actor for StopRecorderActor {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.lifecycle.lock().push("pre_start");
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.lifecycle.lock().push("post_stop");
    Ok(())
  }
}

fn register_cell(system: &ActorSystem, pid: Pid, name: &str, props: &Props) -> ArcShared<ActorCell> {
  let cell = ActorCell::create(system.state(), pid, None, name.to_string(), props).expect("create actor cell");
  system.state().register_cell(cell.clone());
  cell
}

#[test]
fn work_pulling_producer_controller_factory_methods_compile() {
  fn _assert_clone<T: Clone>() {}
  _assert_clone::<WorkPullingProducerControllerCommand<String>>();

  let key = ServiceKey::<ConsumerControllerCommand<u32>>::new("test-workers");
  let _behavior = WorkPullingProducerController::behavior::<u32>("test-producer", key);
}

#[test]
fn work_pulling_producer_controller_with_settings_compiles() {
  let key = ServiceKey::<ConsumerControllerCommand<u32>>::new("test-workers");
  let settings = WorkPullingProducerControllerConfig::new();
  let _behavior = WorkPullingProducerController::behavior_with_settings::<u32>("test-producer", key, &settings, None);
}

#[test]
fn durable_queue_store_is_triggered_before_worker_delivery() {
  let mut state = WorkPullingState::<u32>::new("test-producer".to_string(), 16);
  state.durable_queue = Some(make_typed_ref::<DurableProducerQueueCommand<u32>>());
  state.store_ack_adapter = Some(make_typed_ref());
  state.workers.insert(10, WorkerEntry {
    producer_id:         "test-producer-worker-10".to_string(),
    producer_controller: make_typed_ref::<ProducerControllerCommand<u32>>(),
    next_seq_nr:         1,
    confirmed_seq_nr:    0,
    in_flight:           BTreeMap::new(),
    has_demand:          true,
  });

  let mut deferred = Vec::new();
  collect_on_msg(&mut state, 42_u32, &mut deferred);

  assert_eq!(state.current_seq_nr, 2);
  assert!(state.pending_stores.contains_key(&1));
  assert!(matches!(deferred.as_slice(), [WppcDeferredAction::TellDurableQueue {
    message: DurableProducerQueueCommand::StoreMessageSent { .. },
    timeout: Some(WppcDurableQueueTimeout::Store { seq_nr: 1, attempt: 1 }),
    ..
  }]));
}

#[test]
fn durable_queue_store_ack_keeps_replayable_payload_in_flight() {
  let mut state = WorkPullingState::<u32>::new("test-producer".to_string(), 16);
  let self_ref = make_typed_ref::<WorkPullingProducerControllerCommand<u32>>();
  state.pending_stores.insert(9, PendingDurableStore {
    message:                99_u32,
    worker_key:             10,
    worker_local_seq_nr:    2,
    confirmation_qualifier: "test-producer-worker-10".to_string(),
    replay_confirmation_of: None,
  });
  state.workers.insert(10, WorkerEntry {
    producer_id:         "test-producer-worker-10".to_string(),
    producer_controller: make_typed_ref::<ProducerControllerCommand<u32>>(),
    next_seq_nr:         3,
    confirmed_seq_nr:    0,
    in_flight:           BTreeMap::new(),
    has_demand:          false,
  });

  let mut deferred = Vec::new();
  collect_on_durable_queue_message_stored(&mut state, &StoreMessageSentAck::new(9), &self_ref, &mut deferred);

  let stored = state
    .workers
    .get(&10)
    .and_then(|entry| entry.in_flight.get(&2))
    .expect("stored payload should remain replayable until confirmation");
  assert_eq!(stored.seq_nr(), 9);
  assert_eq!(stored.message(), &99_u32);
  assert!(matches!(
    deferred.as_slice(),
    [WppcDeferredAction::TellWorker {
      worker_key,
      worker_local_seq_nr,
      message: ProducerControllerCommand(command),
      ..
    }]
      if *worker_key == 10
        && *worker_local_seq_nr == 2
        && matches!(command, ProducerControllerCommandKind::Msg { .. })
  ));
}

#[test]
fn direct_worker_delivery_tracks_worker_ack_timeout() {
  let mut state = WorkPullingState::<u32>::new("test-producer".to_string(), 16);
  state.workers.insert(10, WorkerEntry {
    producer_id:         "test-producer-worker-10".to_string(),
    producer_controller: make_typed_ref::<ProducerControllerCommand<u32>>(),
    next_seq_nr:         1,
    confirmed_seq_nr:    0,
    in_flight:           BTreeMap::new(),
    has_demand:          true,
  });

  let mut deferred = Vec::new();
  collect_on_msg(&mut state, 42_u32, &mut deferred);

  assert!(matches!(
    deferred.as_slice(),
    [WppcDeferredAction::TellWorker {
      worker_key,
      worker_local_seq_nr,
      message: ProducerControllerCommand(command),
      ..
    }]
      if *worker_key == 10
        && *worker_local_seq_nr == 1
        && matches!(command, ProducerControllerCommandKind::Msg { .. })
  ));
  let inflight = state
    .workers
    .get(&10)
    .and_then(|entry| entry.in_flight.get(&1))
    .expect("worker entry should track in-flight delivery after collect_on_msg");
  assert_eq!(inflight.seq_nr(), 1);
  assert_eq!(*inflight.message(), 42_u32);
}

#[test]
fn internal_demand_confirm_updates_durable_queue() {
  let mut state = WorkPullingState::<u32>::new("test-producer".to_string(), 16);
  state.durable_queue = Some(make_typed_ref::<DurableProducerQueueCommand<u32>>());
  state.pending_stores.insert(3, PendingDurableStore {
    message:                7_u32,
    worker_key:             10,
    worker_local_seq_nr:    2,
    confirmation_qualifier: "test-producer-worker-10".to_string(),
    replay_confirmation_of: None,
  });
  state.workers.insert(10, WorkerEntry {
    producer_id:         "test-producer-worker-10".to_string(),
    producer_controller: make_typed_ref::<ProducerControllerCommand<u32>>(),
    next_seq_nr:         3,
    confirmed_seq_nr:    0,
    in_flight:           BTreeMap::from([
      (1_u64, MessageSent::new(1_u64, 5_u32, false, "test-producer-worker-10".to_string(), 0)),
      (2_u64, MessageSent::new(3_u64, 7_u32, false, "test-producer-worker-10".to_string(), 0)),
    ]),
    has_demand:          false,
  });

  let request =
    ProducerControllerRequestNext::new("test-producer-worker-10".to_string(), 3, 2, make_typed_ref::<u32>());
  let mut deferred = Vec::new();
  collect_on_internal_demand(&mut state, &request, &mut deferred);

  assert!(matches!(
    deferred
      .iter()
      .find(|action| matches!(
        action,
        WppcDeferredAction::TellDurableQueue {
          message: DurableProducerQueueCommand::StoreMessageConfirmed { .. },
          ..
        }
      )),
    Some(WppcDeferredAction::TellDurableQueue {
      message: DurableProducerQueueCommand::StoreMessageConfirmed { seq_nr, confirmation_qualifier, .. },
      ..
    })
      if *seq_nr == 3 && confirmation_qualifier == "test-producer-worker-10"
  ));
}

#[test]
fn worker_removal_replays_unconfirmed_messages_to_self() {
  let mut state = WorkPullingState::<u32>::new("test-producer".to_string(), 16);
  let worker_ref = crate::test_support::actor_ref_with_sender(Pid::new(10, 0), NullSender);
  let self_ref = make_typed_ref::<WorkPullingProducerControllerCommand<u32>>();
  let listing = Listing::new("test-workers", TypeId::of::<ConsumerControllerCommand<u32>>(), vec![]);
  state.workers.insert(worker_ref.pid().value(), WorkerEntry {
    producer_id:         "test-producer-worker-10".to_string(),
    producer_controller: make_typed_ref::<ProducerControllerCommand<u32>>(),
    next_seq_nr:         3,
    confirmed_seq_nr:    0,
    in_flight:           BTreeMap::from([
      (1_u64, MessageSent::new(11_u64, 41_u32, false, "test-producer-worker-10".to_string(), 0)),
      (2_u64, MessageSent::new(12_u64, 42_u32, false, "test-producer-worker-10".to_string(), 0)),
    ]),
    has_demand:          false,
  });

  let mut deferred = Vec::new();
  collect_on_worker_listing(
    &mut state,
    &listing,
    "test-producer",
    &self_ref,
    &ProducerControllerConfig::new(),
    &mut deferred,
  );

  assert!(state.workers.is_empty());
  assert!(matches!(deferred.first(), Some(WppcDeferredAction::StopWorkerPc(_))));
  assert!(matches!(
    deferred.get(1),
    Some(WppcDeferredAction::TellSelf(_, WorkPullingProducerControllerCommand(command)))
      if matches!(
        command,
        WorkPullingProducerControllerCommandKind::ReplayStoredMessage { sent }
          if sent.message() == &41_u32
      )
  ));
  assert!(matches!(
    deferred.get(2),
    Some(WppcDeferredAction::TellSelf(_, WorkPullingProducerControllerCommand(command)))
      if matches!(
        command,
        WorkPullingProducerControllerCommandKind::ReplayStoredMessage { sent }
          if sent.message() == &42_u32
      )
  ));
}

#[test]
fn worker_spawn_propagates_nested_producer_controller_settings() {
  let mut state = WorkPullingState::<u32>::new("test-producer".to_string(), 16);
  state.demand_adapter = Some(make_typed_ref::<ProducerControllerRequestNext<u32>>());
  let self_ref = make_typed_ref::<WorkPullingProducerControllerCommand<u32>>();
  let worker_ref = crate::test_support::actor_ref_with_sender(Pid::new(11, 0), NullSender);
  let listing = Listing::new("test-workers", TypeId::of::<ConsumerControllerCommand<u32>>(), vec![worker_ref]);
  let mut deferred = Vec::new();
  let producer_settings = ProducerControllerConfig::new().with_durable_queue_retry_attempts(3);

  collect_on_worker_listing(&mut state, &listing, "test-producer", &self_ref, &producer_settings, &mut deferred);

  assert!(matches!(
    deferred.last(),
    Some(WppcDeferredAction::SpawnWorker { producer_controller_settings, .. })
      if producer_controller_settings.durable_queue_retry_attempts() == 3
  ));
}

#[test]
fn durable_queue_send_failure_stops_work_pulling_controller() {
  let system = fraktor_actor_adaptor_std_rs::system::create_noop_actor_system();
  let pid = system.allocate_pid();
  let lifecycle = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let lifecycle = lifecycle.clone();
    move || StopRecorderActor::new(lifecycle.clone())
  });
  let cell = register_cell(&system, pid, "wppc-stop-recorder", &props);
  cell.new_dispatcher_shared().system_dispatch(&cell, SystemMessage::Create).expect("create");

  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::<WorkPullingProducerControllerCommand<u32>>::from_untyped(&mut context, None);
  let state =
    SharedLock::new_with_driver::<SpinSyncMutex<_>>(WorkPullingState::<u32>::new("test-producer".to_string(), 16));
  let durable_queue =
    TypedActorRef::from_untyped(crate::test_support::actor_ref_with_sender(Pid::new(999, 0), FailingSender));

  execute_wppc_deferred(
    vec![WppcDeferredAction::TellDurableQueue {
      target:  durable_queue,
      message: DurableProducerQueueCommand::store_message_confirmed(1, "worker-1".to_string(), 0),
      timeout: None,
    }],
    &mut typed_ctx,
    &state,
    Duration::from_millis(1),
    Duration::from_millis(1),
  );

  assert_eq!(lifecycle.lock().as_slice(), &["pre_start", "post_stop"]);
}
