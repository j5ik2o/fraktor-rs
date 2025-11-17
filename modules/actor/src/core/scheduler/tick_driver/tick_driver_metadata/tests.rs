//! Tests for tick driver metadata.

use core::time::Duration;

use fraktor_utils_rs::core::time::TimerInstant;

use crate::core::scheduler::{AutoProfileKind, TickDriverId, TickDriverMetadata};

#[test]
fn test_tick_driver_id_creation() {
  let id = TickDriverId::new(42);
  assert_eq!(id.as_u64(), 42);
}

#[test]
fn test_tick_driver_id_equality() {
  let id1 = TickDriverId::new(1);
  let id2 = TickDriverId::new(1);
  let id3 = TickDriverId::new(2);

  assert_eq!(id1, id2);
  assert_ne!(id1, id3);
}

#[test]
fn test_tick_driver_metadata_creation() {
  let driver_id = TickDriverId::new(1);
  let resolution = Duration::from_micros(1000);
  let start = TimerInstant::from_ticks(100, resolution);
  let metadata = TickDriverMetadata::new(driver_id, start);

  assert_eq!(metadata.driver_id, driver_id);
  assert_eq!(metadata.start_instant, start);
  assert_eq!(metadata.ticks_total, 0);
}

#[test]
fn test_auto_profile_kind_equality() {
  assert_eq!(AutoProfileKind::Tokio, AutoProfileKind::Tokio);
  assert_ne!(AutoProfileKind::Tokio, AutoProfileKind::Embassy);
}
