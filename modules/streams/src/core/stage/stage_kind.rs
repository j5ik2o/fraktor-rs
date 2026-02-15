/// Minimal stage kinds used by the DSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageKind {
  /// Source that emits a single element.
  SourceSingle,
  /// Flow stage that maps elements.
  FlowMap,
  /// Flow stage that maps elements in async map context.
  FlowMapAsync,
  /// Flow stage that creates a stateful mapper per materialization.
  FlowStatefulMap,
  /// Flow stage that creates a stateful map-concat mapper per materialization.
  FlowStatefulMapConcat,
  /// Flow stage that expands each element into zero or more elements.
  FlowMapConcat,
  /// Flow stage that emits only present mapped elements.
  FlowMapOption,
  /// Flow stage that filters elements by predicate.
  FlowFilter,
  /// Flow stage that drops the first `n` elements.
  FlowDrop,
  /// Flow stage that passes only the first `n` elements.
  FlowTake,
  /// Flow stage that drops elements while predicate matches.
  FlowDropWhile,
  /// Flow stage that passes elements while predicate matches.
  FlowTakeWhile,
  /// Flow stage that passes elements until predicate matches (inclusive).
  FlowTakeUntil,
  /// Flow stage that groups elements into fixed-size chunks.
  FlowGrouped,
  /// Flow stage that emits a sliding window over elements.
  FlowSliding,
  /// Flow stage that emits running accumulation.
  FlowScan,
  /// Flow stage that injects markers between elements and at boundaries.
  FlowIntersperse,
  /// Flow stage that concatenates sub-streams.
  FlowFlatMapConcat,
  /// Flow stage that merges sub-streams up to a configured breadth.
  FlowFlatMapMerge,
  /// Flow stage that buffers upstream elements with an overflow strategy.
  FlowBuffer,
  /// Flow stage that limits in-flight elements with bounded buffering.
  FlowThrottle,
  /// Flow stage that delays each element by a fixed number of ticks.
  FlowDelay,
  /// Flow stage that delays stream start by a fixed number of ticks.
  FlowInitialDelay,
  /// Flow stage that forwards elements only within configured tick window.
  FlowTakeWithin,
  /// Flow stage that represents an asynchronous execution boundary.
  FlowAsyncBoundary,
  /// Flow stage that groups elements into fixed-size batches.
  FlowBatch,
  /// Flow stage that annotates elements with a substream key.
  FlowGroupBy,
  /// Flow stage that recovers from error payloads with a fallback element.
  FlowRecover,
  /// Flow stage that recovers from error payloads with a bounded retry budget.
  FlowRecoverWithRetries,
  /// Flow stage that splits input before elements matching a predicate.
  FlowSplitWhen,
  /// Flow stage that splits input after elements matching a predicate.
  FlowSplitAfter,
  /// Flow stage that merges emitted substreams into a single stream.
  FlowMergeSubstreams,
  /// Flow stage that merges emitted substreams with configured parallelism.
  FlowMergeSubstreamsWithParallelism,
  /// Flow stage that concatenates emitted substreams into a single stream.
  FlowConcatSubstreams,
  /// Flow stage that routes each element to one of two output lanes.
  FlowPartition,
  /// Flow stage that splits tuple payload into two output lanes.
  FlowUnzip,
  /// Flow stage that maps payload then splits mapped tuple into two output lanes.
  FlowUnzipWith,
  /// Flow stage that broadcasts each element to multiple outputs.
  FlowBroadcast,
  /// Flow stage that balances elements across outputs.
  FlowBalance,
  /// Flow stage that merges elements from multiple inputs.
  FlowMerge,
  /// Flow stage that interleaves elements from multiple inputs in round-robin order.
  FlowInterleave,
  /// Flow stage that prepends higher-priority input lanes before others.
  FlowPrepend,
  /// Flow stage that zips elements from multiple inputs.
  FlowZip,
  /// Flow stage that zips elements and fills missing lanes after completion.
  FlowZipAll,
  /// Flow stage that pairs each element with an incrementing index.
  FlowZipWithIndex,
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
