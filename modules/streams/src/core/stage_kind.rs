/// Minimal stage kinds used by the DSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageKind {
  /// Source that emits a single element.
  SourceSingle,
  /// Flow stage that maps elements.
  FlowMap,
  /// Flow stage that concatenates sub-streams.
  FlowFlatMapConcat,
  /// Flow stage that merges sub-streams up to a configured breadth.
  FlowFlatMapMerge,
  /// Flow stage that broadcasts each element to multiple outputs.
  FlowBroadcast,
  /// Flow stage that balances elements across outputs.
  FlowBalance,
  /// Flow stage that merges elements from multiple inputs.
  FlowMerge,
  /// Flow stage that zips elements from multiple inputs.
  FlowZip,
  /// Flow stage that concatenates inputs in port order.
  FlowConcat,
  /// Sink that ignores elements.
  SinkIgnore,
  /// Sink that folds elements.
  SinkFold,
  /// Sink that completes with the first element.
  SinkHead,
  /// Sink that completes with the last element.
  SinkLast,
  /// Sink that applies a closure for each element.
  SinkForeach,
  /// Custom stage.
  Custom,
}
