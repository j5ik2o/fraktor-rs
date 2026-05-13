use crate::actor::messaging::{AnyMessage, Identify};

#[test]
fn identify_keeps_correlation_id() {
  let identify = Identify::new(AnyMessage::new(41_u32));
  let correlation_id = identify.correlation_id().payload().downcast_ref::<u32>().expect("u32");
  assert_eq!(*correlation_id, 41);
}
