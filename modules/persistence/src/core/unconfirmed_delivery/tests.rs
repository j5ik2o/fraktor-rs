use core::time::Duration;

use fraktor_actor_rs::core::actor::actor_ref::ActorRefGeneric;
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared, time::TimerInstant};

use crate::core::unconfirmed_delivery::UnconfirmedDelivery;

#[test]
fn unconfirmed_delivery_accessors_return_values() {
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_u32);
  let destination = ActorRefGeneric::null();
  let sender = ActorRefGeneric::null();
  let timestamp = TimerInstant::from_ticks(10, Duration::from_secs(1));

  let delivery =
    UnconfirmedDelivery::<NoStdToolbox>::new(42, destination.clone(), payload.clone(), Some(sender.clone()), timestamp);

  assert_eq!(delivery.delivery_id(), 42);
  assert_eq!(delivery.destination().pid(), destination.pid());
  assert!(delivery.sender().is_some());
  assert_eq!(delivery.timestamp(), timestamp);
  assert!(delivery.payload().is::<u32>());
  assert!(ArcShared::ptr_eq(&delivery.payload_arc(), &payload));
}
