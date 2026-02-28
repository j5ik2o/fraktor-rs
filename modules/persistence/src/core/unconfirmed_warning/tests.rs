use core::time::Duration;

use fraktor_actor_rs::core::actor::actor_ref::ActorRefGeneric;
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared, time::TimerInstant};

use crate::core::{unconfirmed_delivery::UnconfirmedDelivery, unconfirmed_warning::UnconfirmedWarning};

type TB = NoStdToolbox;

#[test]
fn unconfirmed_warning_reports_count() {
  let payload: ArcShared<dyn core::any::Any + Send + Sync> = ArcShared::new(1_u32);
  let warning = UnconfirmedWarning::<TB>::new(vec![UnconfirmedDelivery::new(
    1,
    ActorRefGeneric::null(),
    payload,
    None,
    TimerInstant::from_ticks(0, Duration::from_secs(1)),
    0,
  )]);

  assert_eq!(warning.count(), 1);
  assert_eq!(warning.unconfirmed_deliveries()[0].delivery_id(), 1);
}
