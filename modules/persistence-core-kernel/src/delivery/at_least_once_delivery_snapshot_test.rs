use core::{any::Any, time::Duration};

use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;
use fraktor_utils_core_rs::{sync::ArcShared, time::TimerInstant};

use crate::delivery::{AtLeastOnceDeliverySnapshot, UnconfirmedDelivery};

#[test]
fn snapshot_accessors_return_values() {
  let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(1_u32);
  let timestamp = TimerInstant::from_ticks(0, Duration::from_secs(1));
  let unconfirmed = UnconfirmedDelivery::new(1, ActorRef::null(), payload, None, timestamp, 0);
  let snapshot = AtLeastOnceDeliverySnapshot::new(5, vec![unconfirmed]);

  assert_eq!(snapshot.current_delivery_id(), 5);
  assert_eq!(snapshot.unconfirmed_deliveries().len(), 1);
}
