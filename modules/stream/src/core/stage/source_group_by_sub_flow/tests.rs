use crate::core::{SubstreamCancelStrategy, stage::Source};

#[test]
fn source_group_by_sub_flow_merge_substreams_preserves_repeated_keys() {
  let mut values = Source::from_array([1_u32, 2, 3, 4, 5])
    .group_by(2, |value: &u32| value % 2, SubstreamCancelStrategy::default())
    .expect("group_by")
    .merge_substreams()
    .collect_values()
    .expect("collect_values");
  values.sort_unstable();
  assert_eq!(values, vec![1_u32, 2, 3, 4, 5]);
}
