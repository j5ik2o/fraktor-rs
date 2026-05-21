use crate::EventSeq;

#[test]
fn empty_contains_no_events() {
  let sequence = EventSeq::<u32>::empty();

  assert!(sequence.is_empty());
  assert_eq!(sequence.len(), 0);
  assert_eq!(sequence.into_events(), Vec::<u32>::new());
}

#[test]
fn single_contains_one_event() {
  let sequence = EventSeq::single(10_u32);

  assert_eq!(sequence.len(), 1);
  assert_eq!(sequence.into_events(), vec![10_u32]);
}

#[test]
fn multiple_collapses_empty_and_single_inputs() {
  assert_eq!(EventSeq::<u32>::multiple(Vec::new()), EventSeq::Empty);
  assert_eq!(EventSeq::multiple(vec![10_u32]), EventSeq::Single(10_u32));
  assert_eq!(EventSeq::multiple(vec![10_u32, 20_u32]).into_events(), vec![10_u32, 20_u32]);
}
