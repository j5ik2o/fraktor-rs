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
  redelivery_tick::RedeliveryTick, unconfirmed_delivery::UnconfirmedDelivery,
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

fn create_sender() -> (ActorRefGeneric<TB>, MessageStore) {
  let messages = ArcShared::new(<<NoStdToolbox as RuntimeToolbox>::MutexFamily as SyncMutexFamily>::create(Vec::new()));
  let sender =
    ActorRefGeneric::new(fraktor_actor_rs::core::actor::Pid::new(1, 1), TestSender { messages: messages.clone() });
  (sender, messages)
}

#[test]
fn delivery_ids_increment_and_confirm() {
  let mut delivery = AtLeastOnceDelivery::<TB>::new(AtLeastOnceDeliveryConfig::default());
  let id1 = delivery.next_delivery_id();
  let id2 = delivery.next_delivery_id();

  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_u32);
  let timestamp = TimerInstant::from_ticks(0, Duration::from_secs(1));
  let unconfirmed = UnconfirmedDelivery::new(id1, ActorRefGeneric::null(), payload, None, timestamp);
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
  delivery.add_unconfirmed(UnconfirmedDelivery::new(id, ActorRefGeneric::null(), payload, None, timestamp));

  let snapshot = delivery.get_delivery_snapshot();
  let mut restored = AtLeastOnceDelivery::<TB>::new(AtLeastOnceDeliveryConfig::default());
  restored.set_delivery_snapshot(snapshot);

  assert_eq!(restored.current_delivery_id(), delivery.current_delivery_id());
  assert_eq!(restored.number_of_unconfirmed(), 1);
}

#[test]
fn deliver_sends_message_and_tracks() {
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(1), 5, 5);
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
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(1), 1, 5);
  let mut delivery = AtLeastOnceDelivery::<TB>::new(config);
  let (destination, _store) = create_sender();

  let _ = delivery.deliver(destination.clone(), None, TimerInstant::from_ticks(0, Duration::from_secs(1)), |id| id);
  let result = delivery.deliver(destination, None, TimerInstant::from_ticks(0, Duration::from_secs(1)), |id| id);
  assert!(result.is_err());
}

#[test]
fn deliveries_to_redeliver_respects_burst_limit() {
  let config = AtLeastOnceDeliveryConfig::new(Duration::from_secs(1), 10, 2);
  let mut delivery = AtLeastOnceDelivery::<TB>::new(config);
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_u32);
  let timestamp = TimerInstant::from_ticks(0, Duration::from_secs(1));

  for id in 1..=3 {
    delivery.add_unconfirmed(UnconfirmedDelivery::new(id, ActorRefGeneric::null(), payload.clone(), None, timestamp));
  }

  let redeliver = delivery.deliveries_to_redeliver();
  assert_eq!(redeliver.len(), 2);
  assert_eq!(redeliver[0].delivery_id(), 1);
}

#[test]
fn redelivery_tick_detection() {
  let tick = RedeliveryTick;
  assert!(AtLeastOnceDelivery::<TB>::is_redelivery_tick(&tick));
}
