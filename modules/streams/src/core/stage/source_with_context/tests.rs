use crate::core::{
  StreamNotUsed,
  stage::{Flow, FlowWithContext, Source, SourceWithContext},
};

#[test]
fn should_create_from_source() {
  let source = Source::from(vec![(1_i32, "a"), (2, "b")]);
  let swc = SourceWithContext::from_source(source);
  let inner = swc.as_source();
  let values = inner.collect_values().unwrap();
  assert_eq!(values, vec![(1, "a"), (2, "b")]);
}

#[test]
fn should_map_output_preserving_context() {
  let source = Source::from(vec![(1_i32, "hello"), (2, "world")]);
  let swc = SourceWithContext::from_source(source);
  let mapped = swc.map(|s: &str| s.len());
  let values = mapped.as_source().collect_values().unwrap();
  assert_eq!(values, vec![(1, 5), (2, 5)]);
}

#[test]
fn should_filter_by_value_preserving_context() {
  let source = Source::from(vec![(1_i32, 10), (2, -5), (3, 20)]);
  let swc = SourceWithContext::from_source(source);
  let filtered = swc.filter(|v: &i32| *v > 0);
  let values = filtered.as_source().collect_values().unwrap();
  assert_eq!(values, vec![(1, 10), (3, 20)]);
}

#[test]
fn should_map_context() {
  let source = Source::from(vec![(1_i32, "a"), (2, "b")]);
  let swc = SourceWithContext::from_source(source);
  let mapped = swc.map_context(|ctx: i32| ctx * 10);
  let values = mapped.as_source().collect_values().unwrap();
  assert_eq!(values, vec![(10, "a"), (20, "b")]);
}

#[test]
fn should_compose_via() {
  let fwc: FlowWithContext<i32, &str, usize, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|(ctx, s): (i32, &str)| (ctx, s.len())));
  let swc = SourceWithContext::from_source(Source::from(vec![(1_i32, "hello"), (2, "hi")]));
  let composed = swc.via(fwc);
  let values = composed.as_source().collect_values().unwrap();
  assert_eq!(values, vec![(1, 5), (2, 2)]);
}
