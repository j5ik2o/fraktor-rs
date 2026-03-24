use crate::core::{
  StreamDslError, SubstreamCancelStrategy,
  stage::{Source, sink::Sink},
};

impl<Key, Out, Mat> super::SourceGroupBySubFlow<Key, Out, Mat>
where
  Key: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  pub(crate) fn merge_substreams_with_parallelism(
    self,
    parallelism: usize,
  ) -> Result<Source<Out, Mat>, StreamDslError> {
    if parallelism == 0 {
      return Err(StreamDslError::InvalidArgument {
        name:   "parallelism",
        value:  0,
        reason: "must be greater than zero",
      });
    }
    Ok(self.merge_substreams())
  }

  pub(crate) fn concat_substreams(self) -> Source<Out, Mat> {
    self.merge_substreams()
  }
}

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

#[test]
fn source_group_by_sub_flow_merge_substreams_with_parallelism_preserves_all_elements() {
  // 準備: group_by で偶奇に分割後、parallelism=2 でマージ
  let mut values = Source::from_array([1_u32, 2, 3, 4, 5])
    .group_by(2, |value: &u32| value % 2, SubstreamCancelStrategy::default())
    .expect("group_by")
    .merge_substreams_with_parallelism(2)
    .expect("merge_substreams_with_parallelism")
    .collect_values()
    .expect("collect_values");

  // 検証: 全要素が保持される（順序は不定のためソート）
  values.sort_unstable();
  assert_eq!(values, vec![1_u32, 2, 3, 4, 5]);
}

#[test]
fn source_group_by_sub_flow_concat_substreams_preserves_all_elements() {
  // 準備: group_by で偶奇に分割後、concat でマージ
  let mut values = Source::from_array([1_u32, 2, 3, 4, 5])
    .group_by(2, |value: &u32| value % 2, SubstreamCancelStrategy::default())
    .expect("group_by")
    .concat_substreams()
    .collect_values()
    .expect("collect_values");

  // 検証: 全要素が保持される（順序は不定のためソート）
  values.sort_unstable();
  assert_eq!(values, vec![1_u32, 2, 3, 4, 5]);
}

#[test]
fn source_group_by_sub_flow_to_produces_runnable_graph() {
  // 準備: group_by → to(Sink::ignore()) で RunnableGraph を生成
  let _graph = Source::from_array([1_u32, 2, 3, 4])
    .group_by(2, |value: &u32| value % 2, SubstreamCancelStrategy::default())
    .expect("group_by")
    .to(Sink::ignore());

  // 検証: 型が正しく RunnableGraph が生成される
}

#[test]
fn source_group_by_sub_flow_map_transforms_values_preserving_keys() {
  // 準備: group_by で偶奇に分割 → map で10倍 → merge
  let mut values = Source::from_array([1_u32, 2, 3, 4])
    .group_by(2, |value: &u32| value % 2, SubstreamCancelStrategy::default())
    .expect("group_by")
    .map(|value| value * 10)
    .merge_substreams()
    .collect_values()
    .expect("collect_values");

  // 検証: 全要素が10倍される（順序は不定のためソート）
  values.sort_unstable();
  assert_eq!(values, vec![10_u32, 20, 30, 40]);
}

#[test]
fn source_group_by_sub_flow_filter_removes_values_preserving_keys() {
  // 準備: group_by で偶奇に分割 → filter で偶数のみ通過 → merge
  let mut values = Source::from_array([1_u32, 2, 3, 4, 5, 6])
    .group_by(2, |value: &u32| value % 2, SubstreamCancelStrategy::default())
    .expect("group_by")
    .filter(|value| value % 2 == 0)
    .merge_substreams()
    .collect_values()
    .expect("collect_values");

  // 検証: 偶数のみが残る（順序は不定のためソート）
  values.sort_unstable();
  assert_eq!(values, vec![2_u32, 4, 6]);
}

#[test]
fn source_group_by_sub_flow_map_then_filter_chains_correctly() {
  // 準備: group_by → map(10倍) → filter(20の倍数) → merge
  let mut values = Source::from_array([1_u32, 2, 3, 4])
    .group_by(2, |value: &u32| value % 2, SubstreamCancelStrategy::default())
    .expect("group_by")
    .map(|value| value * 10)
    .filter(|value| value % 20 == 0)
    .merge_substreams()
    .collect_values()
    .expect("collect_values");

  // 検証: 偶数の10倍（20, 40）のみが20の倍数として残る
  values.sort_unstable();
  assert_eq!(values, vec![20_u32, 40]);
}
