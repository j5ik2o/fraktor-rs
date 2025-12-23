/// Minimal stage kinds used by the DSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageKind {
  /// Source that emits a single element.
  SourceSingle,
  /// Flow stage that maps elements.
  FlowMap,
  /// Flow stage that concatenates sub-streams.
  FlowFlatMapConcat,
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
