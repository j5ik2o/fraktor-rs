use core::time::Duration;

use fraktor_actor_rs::core::actor::actor_ref::ActorRefGeneric;
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared, time::TimerInstant};

use crate::core::{
  at_least_once_delivery_snapshot::AtLeastOnceDeliverySnapshot, unconfirmed_delivery::UnconfirmedDelivery,
};

#[test]
fn snapshot_accessors_return_values() {
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_u32);
  let timestamp = TimerInstant::from_ticks(0, Duration::from_secs(1));
  let unconfirmed = UnconfirmedDelivery::new(1, ActorRefGeneric::null(), payload, None, timestamp);
  let snapshot = AtLeastOnceDeliverySnapshot::<NoStdToolbox>::new(5, vec![unconfirmed]);

  assert_eq!(snapshot.current_delivery_id(), 5);
  assert_eq!(snapshot.unconfirmed_deliveries().len(), 1);
}
