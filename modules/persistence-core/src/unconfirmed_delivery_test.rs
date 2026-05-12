use core::{any::Any, time::Duration};

use fraktor_actor_core_kernel_rs::actor::actor_ref::ActorRef;
use fraktor_utils_core_rs::{sync::ArcShared, time::TimerInstant};

use crate::unconfirmed_delivery::UnconfirmedDelivery;

#[test]
fn unconfirmed_delivery_accessors_return_values() {
  let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(1_u32);
  let destination = ActorRef::null();
  let sender = ActorRef::null();
  let timestamp = TimerInstant::from_ticks(10, Duration::from_secs(1));

  let delivery = UnconfirmedDelivery::new(42, destination.clone(), payload.clone(), Some(sender.clone()), timestamp, 3);

  assert_eq!(delivery.delivery_id(), 42);
  assert_eq!(delivery.destination().pid(), destination.pid());
  assert!(delivery.sender().is_some());
  assert_eq!(delivery.timestamp(), timestamp);
  assert_eq!(delivery.attempt(), 3);
  assert!(delivery.payload().is::<u32>());
  assert!(ArcShared::ptr_eq(&delivery.payload_arc(), &payload));
}
