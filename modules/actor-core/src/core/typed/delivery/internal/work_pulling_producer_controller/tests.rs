use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::time::Duration;

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedLock, SpinSyncMutex};

use super::*;
use crate::core::{
  kernel::{
    actor::{
      Actor, ActorCell, ActorContext, Pid,
      actor_ref::{ActorRef, ActorRefSender, NullSender, SendOutcome},
      error::{ActorError, SendError},
      messaging::{AnyMessage, AnyMessageView, system_message::SystemMessage},
      props::Props,
    },
    system::ActorSystem,
  },
  typed::{
    TypedActorRef,
    actor::TypedActorContext,
    delivery::{
      ConsumerControllerCommand, DurableProducerQueueCommand, ProducerControllerCommand, StoreMessageSentAck,
      WorkPullingProducerControllerCommand, WorkPullingProducerControllerConfig,
    },
    receptionist::ServiceKey,
  },
};

fn make_typed_ref<M: Send + Sync + 'static>() -> TypedActorRef<M> {
  TypedActorRef::from_untyped(ActorRef::new_with_builtin_lock(Pid::new(1, 0), NullSender))
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
      message: ProducerControllerCommand(_),
      ..
    }]
      if *worker_key == 10 && *worker_local_seq_nr == 2
  ));
}

#[test]
fn durable_queue_send_failure_stops_work_pulling_controller() {
  let system = ActorSystem::new_empty();
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
  let durable_queue = TypedActorRef::from_untyped(ActorRef::new_with_builtin_lock(Pid::new(999, 0), FailingSender));

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
