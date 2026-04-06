use core::any::Any;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::event_seq::EventSeq;

#[test]
fn event_seq_empty_reports_zero_length() {
  let sequence = EventSeq::empty();

  assert!(sequence.is_empty());
  assert_eq!(sequence.len(), 0);
  assert!(sequence.into_events().is_empty());
}

#[test]
fn event_seq_single_contains_one_payload() {
  let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(3_i32);
  let sequence = EventSeq::single(payload);

  assert!(!sequence.is_empty());
  assert_eq!(sequence.len(), 1);
  match sequence {
    | EventSeq::Single(value) => assert_eq!(value.downcast_ref::<i32>(), Some(&3_i32)),
    | _ => panic!("expected single payload"),
  }
}

#[test]
fn event_seq_multiple_keeps_payload_order() {
  let v0: ArcShared<dyn Any + Send + Sync> = ArcShared::new(1_i32);
  let v1: ArcShared<dyn Any + Send + Sync> = ArcShared::new(2_i32);
  let v2: ArcShared<dyn Any + Send + Sync> = ArcShared::new(3_i32);
  let sequence = EventSeq::multiple(vec![v0, v1, v2]);

  assert_eq!(sequence.len(), 3);
  let values = sequence.into_events();
  assert_eq!(values.len(), 3);
  assert_eq!(values[0].downcast_ref::<i32>(), Some(&1_i32));
  assert_eq!(values[1].downcast_ref::<i32>(), Some(&2_i32));
  assert_eq!(values[2].downcast_ref::<i32>(), Some(&3_i32));
}
