use crate::core::dsl::{Source, TailSource};

#[test]
fn tail_source_collects_wrapped_values() {
  let tail_source = TailSource::new(Source::from_array([1_u32, 2, 3]));
  let values = tail_source.collect_values().expect("collect_values");
  assert_eq!(values, vec![1_u32, 2_u32, 3_u32]);
}

#[test]
fn tail_source_into_source_returns_underlying_source() {
  let tail_source = TailSource::new(Source::single(7_u32));
  let source = tail_source.into_source();
  let values = source.collect_values().expect("collect_values");
  assert_eq!(values, vec![7_u32]);
}
