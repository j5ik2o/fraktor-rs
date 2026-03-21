use crate::core::{
  StreamNotUsed, SubstreamCancelStrategy,
  stage::{Source, flow::Flow},
};

#[test]
fn flow_group_by_sub_flow_merge_substreams_preserves_repeated_keys() {
  let mut values = Source::from_array([1_u32, 2, 3, 4, 5])
    .via(
      Flow::<u32, u32, StreamNotUsed>::new()
        .group_by(2, |value: &u32| value % 2, SubstreamCancelStrategy::default())
        .expect("group_by")
        .merge_substreams(),
    )
    .collect_values()
    .expect("collect_values");
  values.sort_unstable();
  assert_eq!(values, vec![1_u32, 2, 3, 4, 5]);
}
