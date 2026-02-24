use crate::core::{
  StreamNotUsed,
  stage::{Flow, FlowWithContext, Source},
};

#[test]
fn should_map_output_preserving_context() {
  let fwc: FlowWithContext<i32, &str, usize, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v)).map(|s: &str| s.len());
  let values = Source::from(vec![(1_i32, "hello"), (2, "world")]).via(fwc.as_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(1, 5), (2, 5)]);
}

#[test]
fn should_filter_by_value_preserving_context() {
  let fwc: FlowWithContext<i32, i32, i32, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, i32)| v)).filter(|v: &i32| *v > 0);
  let values = Source::from(vec![(1_i32, 10), (2, -5), (3, 20)]).via(fwc.as_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(1, 10), (3, 20)]);
}

#[test]
fn should_map_context() {
  // Ctx=i32, Ctx2=i64 — different types ensure map_context cannot be a no-op
  let fwc: FlowWithContext<i32, &str, &str, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v));
  // forward and reverse are NOT inverses: output ctx differs from input ctx
  let mapped = fwc.map_context(|ctx: i32| i64::from(ctx) * 10, |ctx2: i64| (ctx2 as i32) - 1);
  // Input: (10_i64, "a"), (20_i64, "b")
  // → reverse(10) = 9, reverse(20) = 19
  // → inner (identity): (9, "a"), (19, "b")
  // → forward(9) = 90, forward(19) = 190
  let values = Source::from(vec![(10_i64, "a"), (20_i64, "b")]).via(mapped.as_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(90_i64, "a"), (190_i64, "b")]);
}

#[test]
fn should_compose_via() {
  let fwc1: FlowWithContext<i32, &str, &str, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|v: (i32, &str)| v));
  let fwc2: FlowWithContext<i32, &str, usize, StreamNotUsed> =
    FlowWithContext::from_flow(Flow::new().map(|(ctx, s): (i32, &str)| (ctx, s.len())));
  let composed = fwc1.via(fwc2);
  let values = Source::from(vec![(1_i32, "hello"), (2, "hi")]).via(composed.as_flow()).collect_values().unwrap();
  assert_eq!(values, vec![(1, 5), (2, 2)]);
}
