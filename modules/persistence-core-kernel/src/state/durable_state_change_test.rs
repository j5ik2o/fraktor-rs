use alloc::string::ToString;

use crate::state::DurableStateChange;

#[test]
fn durable_state_change_exposes_metadata_and_value() {
  let change = DurableStateChange::new(3, "pid-1".to_string(), 7, "orders".to_string(), 42);

  assert_eq!(change.offset(), 3);
  assert_eq!(change.persistence_id(), "pid-1");
  assert_eq!(change.revision(), 7);
  assert_eq!(change.tag(), "orders");
  assert_eq!(change.value(), &42);
  assert_eq!(change.into_value(), 42);
}
