use core::time::Duration;

use fraktor_actor_rs::core::{
  actor::actor_ref::{ActorRefGeneric, ActorRefSender, SendOutcome},
  error::SendError,
  messaging::AnyMessageGeneric,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::ArcShared,
  time::TimerInstant,
};

use crate::core::{
  at_least_once_delivery::AtLeastOnceDelivery, at_least_once_delivery_config::AtLeastOnceDeliveryConfig,
  persistence_error::PersistenceError, redelivery_tick::RedeliveryTick, unconfirmed_delivery::UnconfirmedDelivery,
};

type TB = NoStdToolbox;
type MessageStore = ArcShared<ToolboxMutex<Vec<AnyMessageGeneric<TB>>, TB>>;

struct TestSender {
  messages: MessageStore,
}

impl ActorRefSender<TB> for TestSender {
  fn send(&mut self, message: AnyMessageGeneric<TB>) -> Result<SendOutcome, SendError<TB>> {
    self.messages.lock().push(message);
    Ok(SendOutcome::Delivered)
  }
}

struct FailingSender;

impl ActorRefSender<TB> for FailingSender {
  fn send(&mut self, message: AnyMessageGeneric<TB>) -> Result<SendOutcome, SendError<TB>> {
    Err(SendError::closed(message))
  }
}

fn create_sender() -> (ActorRefGeneric<TB>, MessageStore) {
  let messages = ArcShared::new(<<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()));
  let sender =
    ActorRefGeneric::new(fraktor_actor_rs::core::actor::Pid::new(1, 1), TestSender { messages: messages.clone() });
  (sender, messages)
}

fn create_failing_sender() -> ActorRefGeneric<TB> {
  ActorRefGeneric::new(fraktor_actor_rs::core::actor::Pid::new(1, 2), FailingSender)
}

#[test]
fn delivery_ids_increment_and_confirm() {
  let mut delivery = AtLeastOnceDelivery::<TB>::new(AtLeastOnceDeliveryConfig::default());
  let id1 = delivery.next_delivery_id();
  let id2 = delivery.next_delivery_id();

  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_u32);
  let timestamp = TimerInstant::from_ticks(0, Duration::from_secs(1));
  let unconfirmed = UnconfirmedDelivery::new(id1, ActorRefGeneric::null(), payload, None, timestamp, 0);
  delivery.add_unconfirmed(unconfirmed);

  assert_eq!(id1, 1);
  assert_eq!(id2, 2);
  assert_eq!(delivery.current_delivery_id(), 3);
  assert!(delivery.confirm_delivery(id1));
  assert_eq!(delivery.number_of_unconfirmed(), 0);
}

#[test]
fn delivery_snapshot_roundtrip() {
  let mut delivery = AtLeastOnceDelivery::<TB>::new(AtLeastOnceDeliveryConfig::default());
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new("data");
  let timestamp = TimerInstant::from_ticks(1, Duration::from_secs(1));
  let id = delivery.next_delivery_id();
  delivery.add_unconfirmed(UnconfirmedDelivery::new(id, ActorRefGeneric::null(), payload, None, timestamp, 9));

  let snapshot = delivery.get_delivery_snapshot();
  let mut restored = AtLeastOnceDelivery::<TB>::new(AtLeastOnceDeliveryConfig::default());
  let restore_now = TimerInstant::from_ticks(20, Duration::from_secs(1));
  restored.set_delivery_snapshot(snapshot, restore_now);

  assert_eq!(restored.current_delivery_id(), delivery.current_delivery_id());
  assert_eq!(restored.number_of_unconfirmed(), 1);
  assert_eq!(restored.unconfirmed_deliveries()[0].attempt(), 0);
  assert_eq!(restored.unconfirmed_deliveries()[0].timestamp(), TimerInstant::from_ticks(10, Duration::from_secs(1)));
}

#[test]
fn deliver_sends_message_and_tracks() {
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(1), 5, 5, 5);
  let mut delivery = AtLeastOnceDelivery::<TB>::new(config);
  let (destination, store) = create_sender();

  let id = delivery
    .deliver(destination.clone(), None, TimerInstant::from_ticks(0, Duration::from_secs(1)), |id| id)
    .expect("delivery should succeed");

  assert_eq!(id, 1);
  assert_eq!(delivery.number_of_unconfirmed(), 1);
  let messages = store.lock();
  assert_eq!(messages.len(), 1);
  let payload = messages[0].payload().downcast_ref::<u64>().expect("payload type");
  assert_eq!(*payload, 1);
}

#[test]
fn deliver_rejects_when_max_unconfirmed_exceeded() {
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(1), 1, 5, 5);
  let mut delivery = AtLeastOnceDelivery::<TB>::new(config);
  let (destination, _store) = create_sender();

  let _ = delivery.deliver(destination.clone(), None, TimerInstant::from_ticks(0, Duration::from_secs(1)), |id| id);
  let result = delivery.deliver(destination, None, TimerInstant::from_ticks(0, Duration::from_secs(1)), |id| id);
  assert!(result.is_err());
}

#[test]
fn deliveries_to_redeliver_respects_burst_limit() {
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(1), 10, 2, 5);
  let mut delivery = AtLeastOnceDelivery::<TB>::new(config);
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_u32);
  let timestamp = TimerInstant::from_ticks(0, Duration::from_secs(1));

  for id in 1..=3 {
    delivery.add_unconfirmed(UnconfirmedDelivery::new(
      id,
      ActorRefGeneric::null(),
      payload.clone(),
      None,
      timestamp,
      1,
    ));
  }

  let redeliver = delivery.deliveries_to_redeliver(TimerInstant::from_ticks(1, Duration::from_secs(1)));
  assert_eq!(redeliver.len(), 2);
  assert_eq!(redeliver[0].delivery_id(), 1);
}

#[test]
fn redelivery_tick_detection() {
  let tick = RedeliveryTick;
  assert!(AtLeastOnceDelivery::<TB>::is_redelivery_tick(&tick));
}

#[test]
fn redelivery_tick_emits_warning_only_when_attempt_reaches_threshold() {
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(1), 10, 2, 1);
  let mut delivery = AtLeastOnceDelivery::<TB>::new(config);
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_u32);
  let (destination, _store) = create_sender();
  let resolution = Duration::from_secs(1);
  let timestamp = TimerInstant::from_ticks(0, resolution);
  delivery.add_unconfirmed(UnconfirmedDelivery::new(1, destination, payload, None, timestamp, 1));

  let tick = RedeliveryTick;
  let warning = delivery
    .handle_message(&tick, TimerInstant::from_ticks(1, resolution))
    .expect("redelivery should succeed")
    .expect("warning should be emitted at threshold");
  assert_eq!(warning.count(), 1);
  assert_eq!(warning.unconfirmed_deliveries()[0].delivery_id(), 1);

  let no_warning =
    delivery.handle_message(&tick, TimerInstant::from_ticks(2, resolution)).expect("redelivery should succeed");
  assert!(no_warning.is_none());
  assert_eq!(delivery.unconfirmed_deliveries()[0].attempt(), 3);
}

#[test]
fn delivery_snapshot_restore_restarts_warning_counter() {
  let resolution = Duration::from_secs(1);
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(1), 10, 2, 1);
  let mut original = AtLeastOnceDelivery::<TB>::new(config.clone());
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_u32);
  let (destination, _store) = create_sender();
  original.add_unconfirmed(UnconfirmedDelivery::new(
    1,
    destination,
    payload,
    None,
    TimerInstant::from_ticks(3, resolution),
    7,
  ));

  let snapshot = original.get_delivery_snapshot();
  let mut restored = AtLeastOnceDelivery::<TB>::new(config);
  restored.set_delivery_snapshot(snapshot, TimerInstant::from_ticks(10, resolution));

  let tick = RedeliveryTick;
  let first_tick_warning =
    restored.handle_message(&tick, TimerInstant::from_ticks(10, resolution)).expect("redelivery should succeed");
  assert!(first_tick_warning.is_none());

  let second_tick_warning = restored
    .handle_message(&tick, TimerInstant::from_ticks(11, resolution))
    .expect("redelivery should succeed")
    .expect("warning should restart after restore");
  assert_eq!(second_tick_warning.count(), 1);
  assert_eq!(second_tick_warning.unconfirmed_deliveries()[0].delivery_id(), 1);
}

#[test]
fn delivery_snapshot_restore_redelivers_immediately_when_restore_time_is_before_interval() {
  let resolution = Duration::from_secs(1);
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(10), 10, 2, 5);
  let mut original = AtLeastOnceDelivery::<TB>::new(config.clone());
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_u32);
  let (destination, store) = create_sender();
  original.add_unconfirmed(UnconfirmedDelivery::new(
    1,
    destination,
    payload,
    None,
    TimerInstant::from_ticks(3, resolution),
    4,
  ));

  let snapshot = original.get_delivery_snapshot();
  let mut restored = AtLeastOnceDelivery::<TB>::new(config);
  let restore_now = TimerInstant::from_ticks(5, resolution);
  restored.set_delivery_snapshot(snapshot, restore_now);

  let tick = RedeliveryTick;
  let warning = restored.handle_message(&tick, restore_now).expect("redelivery should succeed");
  assert!(warning.is_none());
  assert_eq!(restored.unconfirmed_deliveries()[0].attempt(), 1);
  assert_eq!(restored.unconfirmed_deliveries()[0].timestamp(), restore_now);
  assert_eq!(store.lock().len(), 1);
}

#[test]
fn redelivery_tick_propagates_send_failure() {
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(1), 10, 2, 1);
  let mut delivery = AtLeastOnceDelivery::<TB>::new(config);
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_u32);
  let resolution = Duration::from_secs(1);
  let timestamp = TimerInstant::from_ticks(0, resolution);
  delivery.add_unconfirmed(UnconfirmedDelivery::new(1, create_failing_sender(), payload, None, timestamp, 1));

  let tick = RedeliveryTick;
  let result = delivery.handle_message(&tick, TimerInstant::from_ticks(1, resolution));
  assert!(matches!(
    result,
    Err(PersistenceError::MessagePassing(reason)) if reason.contains("Closed")
  ));
  assert_eq!(delivery.unconfirmed_deliveries()[0].attempt(), 1);
}
