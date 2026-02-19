use crate::core::stage::{BidiFlow, Flow, Source, StreamNotUsed};

fn collect_single(flow: Flow<u32, u32, StreamNotUsed>, value: u32) -> Vec<u32> {
  Source::single(value).via(flow).collect_values().expect("collect_values")
}

#[test]
fn bidi_flow_split_returns_original_fragments() {
  let bidi = BidiFlow::from_flows(Flow::new().map(|value: u32| value + 1), Flow::new().map(|value: u32| value + 10));
  let (top, bottom, mat) = bidi.split();

  assert_eq!(collect_single(top, 1_u32), vec![2_u32]);
  assert_eq!(collect_single(bottom, 1_u32), vec![11_u32]);
  assert_eq!(mat, StreamNotUsed::new());
}

#[test]
fn bidi_flow_identity_passes_through_unchanged() {
  let bidi = BidiFlow::<u32, u32, u32, u32, StreamNotUsed>::identity();
  let (top, bottom, mat) = bidi.split();

  assert_eq!(collect_single(top, 42_u32), vec![42_u32]);
  assert_eq!(collect_single(bottom, 99_u32), vec![99_u32]);
  assert_eq!(mat, StreamNotUsed::new());
}

#[test]
fn bidi_flow_reversed_swaps_top_and_bottom() {
  let bidi = BidiFlow::from_flows(Flow::new().map(|value: u32| value + 1), Flow::new().map(|value: u32| value + 10));
  let reversed = bidi.reversed();
  let (top, bottom, mat) = reversed.split();

  assert_eq!(collect_single(top, 1_u32), vec![11_u32]);
  assert_eq!(collect_single(bottom, 1_u32), vec![2_u32]);
  assert_eq!(mat, StreamNotUsed::new());
}

#[test]
fn bidi_flow_from_functions_builds_top_and_bottom_mappers() {
  let bidi = BidiFlow::from_functions(|value: u32| value + 2, |value: u32| value + 20);
  let (top, bottom, mat) = bidi.split();

  assert_eq!(collect_single(top, 1_u32), vec![3_u32]);
  assert_eq!(collect_single(bottom, 1_u32), vec![21_u32]);
  assert_eq!(mat, StreamNotUsed::new());
}

#[test]
fn bidi_flow_from_function_is_alias_of_from_functions() {
  let bidi = BidiFlow::from_function(|value: u32| value + 3, |value: u32| value + 30);
  let (top, bottom, mat) = bidi.split();

  assert_eq!(collect_single(top, 1_u32), vec![4_u32]);
  assert_eq!(collect_single(bottom, 1_u32), vec![31_u32]);
  assert_eq!(mat, StreamNotUsed::new());
}

#[test]
fn bidi_flow_atop_composes_and_keeps_left_materialized_value() {
  let left = BidiFlow::from_flows_mat(
    Flow::new().map(|value: u32| value + 1),
    Flow::new().map(|value: u32| value.saturating_mul(2)),
    10_u32,
  );
  let right = BidiFlow::from_flows_mat(
    Flow::new().map(|value: u32| value + 10),
    Flow::new().map(|value: u32| value.saturating_sub(3)),
    99_u32,
  );

  let composed = left.atop(right);
  let (top, bottom, mat) = composed.split();

  assert_eq!(collect_single(top, 1_u32), vec![12_u32]);
  assert_eq!(collect_single(bottom, 10_u32), vec![14_u32]);
  assert_eq!(mat, 10_u32);

  let composed_for_mat = BidiFlow::from_flows_mat(
    Flow::new().map(|value: u32| value + 1),
    Flow::new().map(|value: u32| value.saturating_mul(2)),
    10_u32,
  )
  .atop(BidiFlow::from_flows_mat(
    Flow::new().map(|value: u32| value + 10),
    Flow::new().map(|value: u32| value.saturating_sub(3)),
    99_u32,
  ));
  let (graph, materialized) = composed_for_mat.join(Flow::new()).into_parts();
  let _ = graph;
  assert_eq!(materialized, 10_u32);
}

#[test]
fn bidi_flow_join_composes_top_flow_and_bottom_and_keeps_materialized_value() {
  let joined = BidiFlow::from_flows(Flow::new().map(|value: u32| value + 1), Flow::new().map(|value: u32| value + 10))
    .join(Flow::new().map(|value: u32| value + 3));

  let values = Source::single(1_u32).via(joined).collect_values().expect("collect_values");
  assert_eq!(values, vec![15_u32]);

  let (graph, materialized) =
    BidiFlow::from_flows_mat(Flow::new().map(|value: u32| value + 1), Flow::new().map(|value: u32| value + 10), 77_u32)
      .join(Flow::new().map(|value: u32| value + 3))
      .into_parts();
  let _ = graph;
  assert_eq!(materialized, 77_u32);
}

#[test]
fn bidi_flow_split_keeps_materialized_value() {
  let bidi =
    BidiFlow::from_flows_mat(Flow::new().map(|value: u32| value + 1), Flow::new().map(|value: u32| value + 10), 55_u32);
  let (top, bottom, mat) = bidi.split();

  assert_eq!(collect_single(top, 1_u32), vec![2_u32]);
  assert_eq!(collect_single(bottom, 1_u32), vec![11_u32]);
  assert_eq!(mat, 55_u32);
}
