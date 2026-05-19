use fraktor_actor_core_kernel_rs::actor::scheduler::tick_driver::{TickDriver, TickDriverKind};

use super::EmbassyTickDriver;

#[test]
fn embassy_tick_driver_reports_embassy_kind() {
  assert_eq!(EmbassyTickDriver::default().kind(), TickDriverKind::Embassy);
}
