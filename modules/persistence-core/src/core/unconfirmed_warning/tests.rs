use core::time::Duration;

use fraktor_actor_core_rs::core::kernel::actor::actor_ref::ActorRef;
use fraktor_utils_core_rs::core::{sync::ArcShared, time::TimerInstant};

use crate::core::{unconfirmed_delivery::UnconfirmedDelivery, unconfirmed_warning::UnconfirmedWarning};

#[test]
fn unconfirmed_warning_reports_count() {
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_u32);
  let warning = UnconfirmedWarning::new(vec![UnconfirmedDelivery::new(
    1,
    ActorRef::null(),
    payload,
    None,
    TimerInstant::from_ticks(0, Duration::from_secs(1)),
    0,
  )]);

  assert_eq!(warning.count(), 1);
  assert_eq!(warning.unconfirmed_deliveries()[0].delivery_id(), 1);
}
