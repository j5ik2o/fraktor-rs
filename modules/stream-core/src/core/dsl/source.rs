use alloc::{
  boxed::Box,
  collections::{BTreeSet, VecDeque},
  string::String,
  vec::Vec,
};
use core::{
  any::TypeId,
  array::IntoIter,
  future::Future,
  marker::PhantomData,
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use super::{
  BoundedSourceQueue, DynValue, KeepLeft, KeepRight, MatCombine, MatCombineRule, Materialized, Materializer,
  OverflowStrategy, RestartBackoff, RestartConfig, RunnableGraph, SourceDefinition, SourceLogic, SourceQueue,
  SourceQueueWithComplete, StageContext, StageDefinition, StageKind, StatefulMapConcatAccumulator, StreamCompletion,
  StreamDone, StreamDslError, StreamError, StreamGraph, StreamNotUsed, SupervisionStrategy, ThrottleMode,
  flow::{
    Flow, backpressure_timeout_definition, balance_definition, batch_definition, broadcast_definition,
    buffer_definition, completion_timeout_definition, concat_definition, concat_lazy_definition,
    concat_substreams_definition, conflate_with_seed_definition, debounce_definition, delay_definition,
    drop_definition, drop_while_definition, expand_definition, filter_definition, flat_map_concat_definition,
    flat_map_merge_definition, flat_map_prefix_definition, group_by_definition, grouped_definition,
    grouped_weighted_definition, idle_timeout_definition, initial_delay_definition, initial_timeout_definition,
    interleave_definition, intersperse_definition, keep_alive_definition, log_definition, map_async_definition,
    map_concat_definition, map_option_definition, merge_definition, merge_latest_definition,
    merge_preferred_definition, merge_prioritized_definition, merge_sorted_definition, merge_substreams_definition,
    merge_substreams_with_parallelism_definition, or_else_definition, partition_definition, prepend_definition,
    prepend_lazy_definition, sample_definition, scan_definition, sliding_definition, split_after_definition,
    split_after_definition_with_cancel_strategy, split_when_definition, split_when_definition_with_cancel_strategy,
    stateful_map_concat_accumulator_definition, stateful_map_concat_definition, stateful_map_definition,
    stateful_map_with_on_complete_definition, switch_map_definition, take_definition, take_until_definition,
    take_while_definition, take_within_definition, throttle_definition, unzip_definition, unzip_with_definition,
    watch_termination_definition, zip_all_definition, zip_definition, zip_with_index_definition,
  },
  shape::{Inlet, Outlet, StreamShape},
  sink::Sink,
  source_group_by_sub_flow::SourceGroupBySubFlow,
  source_sub_flow::SourceSubFlow,
  source_with_context::SourceWithContext,
  validate_positive_argument,
};
use crate::core::{
  SubstreamCancelStrategy,
  attributes::Attributes,
  r#impl::{
    fusing::{StreamBufferConfig, map_definition},
    interpreter::{DEFAULT_BOUNDARY_CAPACITY, IslandBoundaryShared, IslandSplitter},
    materialization::Stream,
  },
  materialization::DriveOutcome,
  stage::{GraphStage, GraphStageLogic, StreamStage},
};

#[cfg(test)]
mod tests;

/// Source stage definition.
pub struct Source<Out, Mat> {
  graph: StreamGraph,
  mat:   Mat,
  _pd:   PhantomData<fn() -> Out>,
}

impl<Out> Source<Out, StreamNotUsed>
where
  Out: Send + 'static,
{
  /// Creates a source that emits no elements and completes immediately.
  #[must_use]
  pub fn empty() -> Self {
    Self::from_logic(StageKind::Custom, EmptySourceLogic)
  }

  /// Creates a source from an optional element.
  ///
  /// Emits one element when `value` is [`Some`], otherwise completes immediately.
  #[must_use]
  pub fn from_option(value: Option<Out>) -> Self {
    match value {
      | Some(value) => Self::single(value),
      | None => Self::empty(),
    }
  }

  /// Creates a source from an iterator.
  #[must_use]
  pub fn from_iterator<I>(values: I) -> Self
  where
    I: IntoIterator<Item = Out>,
    I::IntoIter: Send + 'static, {
    Self::from_logic(StageKind::Custom, IteratorSourceLogic { values: values.into_iter() })
  }

  /// Compatibility alias of [`Source::from_iterator`].
  #[must_use]
  pub fn from<I>(values: I) -> Self
  where
    I: IntoIterator<Item = Out>,
    I::IntoIter: Send + 'static, {
    Self::from_iterator(values)
  }

  /// Creates a source from an array.
  #[must_use]
  pub fn from_array<const N: usize>(values: [Out; N]) -> Self {
    Self::from_logic(StageKind::Custom, ArraySourceLogic { values: values.into_iter() })
  }

  /// Creates a source that emits a single element.
  #[must_use]
  pub fn single(value: Out) -> Self {
    let mut graph = StreamGraph::new();
    let outlet: Outlet<Out> = Outlet::new();
    let logic = SingleSourceLogic { value: Some(value) };
    let definition = SourceDefinition {
      kind:        StageKind::SourceSingle,
      outlet:      outlet.id(),
      output_type: TypeId::of::<Out>(),
      mat_combine: MatCombine::Right,
      supervision: SupervisionStrategy::Stop,
      restart:     None,
      logic:       Box::new(logic),
      attributes:  Attributes::new(),
    };
    graph.push_stage(StageDefinition::Source(definition));
    Self { graph, mat: StreamNotUsed::new(), _pd: PhantomData }
  }

  /// Creates a source that fails when pulled.
  #[must_use]
  pub fn failed(error: StreamError) -> Self {
    Self::from_logic(StageKind::Custom, FailedSourceLogic { error })
  }

  /// Creates a source that never emits and never completes.
  #[must_use]
  pub fn never() -> Self {
    Self::from_logic(StageKind::Custom, NeverSourceLogic)
  }

  /// Creates a source that repeatedly emits the provided element.
  #[must_use]
  pub fn repeat(value: Out) -> Self
  where
    Out: Clone, {
    Self::from_logic(StageKind::Custom, RepeatSourceLogic { value })
  }

  /// Creates a source that repeatedly cycles over provided values.
  #[must_use]
  pub fn cycle<I>(values: I) -> Self
  where
    I: IntoIterator<Item = Out>,
    Out: Clone, {
    let values = values.into_iter().collect::<Vec<Out>>();
    if values.is_empty() {
      return Self::empty();
    }
    Self::from_logic(StageKind::Custom, CycleSourceLogic { values, index: 0 })
  }

  /// Creates a source that emits an infinite iterative sequence.
  #[must_use]
  pub fn iterate<F>(seed: Out, func: F) -> Self
  where
    Out: Clone,
    F: FnMut(Out) -> Out + Send + Sync + 'static, {
    Self::from_logic(StageKind::Custom, IterateSourceLogic { current: seed, func })
  }

  /// Creates a source from a materializer-provided factory.
  ///
  /// The factory is deferred until the first element is demanded, equivalent to `lazy_source`.
  #[must_use]
  pub fn from_materializer<F>(factory: F) -> Self
  where
    F: FnOnce() -> Self + Send + 'static,
    Out: Sync, {
    Self::lazy_source(factory)
  }

  /// Converts this source into a context-carrying source by attaching unit context.
  #[must_use]
  pub fn into_source_with_context(self) -> SourceWithContext<(), Out, StreamNotUsed>
  where
    Out: Sync, {
    let inner = self.map(|value| ((), value));
    SourceWithContext::from_source(inner)
  }

  /// Creates a sink endpoint that can be paired with a source subscriber bridge.
  #[must_use]
  pub fn as_subscriber() -> Sink<Out, StreamCompletion<StreamDone>>
  where
    Out: Sync, {
    Sink::ignore()
  }

  /// Creates a sink endpoint for actor interop entry points.
  #[must_use]
  pub fn sink() -> Sink<Out, StreamCompletion<StreamDone>>
  where
    Out: Sync, {
    Self::as_subscriber()
  }

  /// Combines multiple sources by merging them into a single output stream.
  ///
  /// Elements from all sources are interleaved as they become available.
  /// The materialized value of the first source is kept (`KeepLeft` semantics).
  ///
  /// Returns an empty source when the iterator yields no elements.
  #[must_use]
  pub fn combine<I>(sources: I) -> Self
  where
    I: IntoIterator<Item = Self>,
    Out: Sync, {
    let mut iter = sources.into_iter();
    let Some(first) = iter.next() else {
      return Self::empty();
    };
    let rest: Vec<Self> = iter.collect();
    if rest.is_empty() {
      return first;
    }
    let fan_in = rest.len().saturating_add(1);
    let (mut graph, mat) = first.into_parts();
    let first_outlet = graph.tail_outlet();
    let mut other_outlets = Vec::new();
    for source in rest {
      let (other_graph, _other_mat) = source.into_parts();
      let other_outlet = other_graph.tail_outlet();
      graph.append_unwired(other_graph);
      if let Some(port) = other_outlet {
        other_outlets.push(port);
      }
    }
    let definition = merge_definition::<Out>(fan_in);
    let merge_inlet = definition.inlet;
    graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = first_outlet {
      graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(merge_inlet), MatCombine::Left);
    }
    for from in other_outlets {
      graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(merge_inlet), MatCombine::Left);
    }
    Source { graph, mat, _pd: PhantomData }
  }

  /// Combines multiple sources using merge-prioritized fan-in.
  ///
  /// Returns an empty source when `sources` is empty.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `priorities.len()` does not match the
  /// number of sources or when any priority is zero.
  pub fn merge_prioritized_n<I>(sources: I, priorities: &[usize]) -> Result<Self, StreamDslError>
  where
    I: IntoIterator<Item = Self>,
    Out: Sync, {
    let mut iter = sources.into_iter();
    let Some(first) = iter.next() else {
      if priorities.is_empty() {
        return Ok(Self::empty());
      }
      return Err(StreamDslError::InvalidArgument {
        name:   "priorities",
        value:  priorities.len(),
        reason: "length must match fan_in",
      });
    };

    let rest: Vec<Self> = iter.collect();
    let fan_in = rest.len().saturating_add(1);
    if priorities.len() != fan_in {
      return Err(StreamDslError::InvalidArgument {
        name:   "priorities",
        value:  priorities.len(),
        reason: "length must match fan_in",
      });
    }
    for (i, &priority) in priorities.iter().enumerate() {
      if priority == 0 {
        return Err(StreamDslError::InvalidArgument {
          name:   "priorities",
          value:  i,
          reason: "all priorities must be positive",
        });
      }
    }
    if rest.is_empty() {
      return Ok(first);
    }

    let (mut graph, mat) = first.into_parts();
    let first_outlet = graph.tail_outlet();
    let mut other_outlets = Vec::with_capacity(rest.len());
    for source in rest {
      let (other_graph, _other_mat) = source.into_parts();
      let other_outlet = other_graph.tail_outlet();
      graph.append_unwired(other_graph);
      if let Some(port) = other_outlet {
        other_outlets.push(port);
      }
    }
    let definition = merge_prioritized_definition::<Out>(fan_in, priorities);
    let merge_inlet = definition.inlet;
    graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = first_outlet {
      graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(merge_inlet), MatCombine::Left);
    }
    for from in other_outlets {
      graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(merge_inlet), MatCombine::Left);
    }
    Ok(Source { graph, mat, _pd: PhantomData })
  }

  /// Creates a source from a Java-stream compatible iterator.
  #[must_use]
  pub fn from_java_stream<I>(values: I) -> Self
  where
    I: IntoIterator<Item = Out>,
    I::IntoIter: Send + 'static, {
    Self::from_iterator(values)
  }

  /// Creates a source from a publisher-compatible iterator.
  #[must_use]
  pub fn from_publisher<I>(values: I) -> Self
  where
    I: IntoIterator<Item = Out>,
    I::IntoIter: Send + 'static, {
    Self::from_iterator(values)
  }

  /// Creates a source from an input-stream compatible iterator.
  #[must_use]
  pub fn from_input_stream<I>(values: I) -> Self
  where
    I: IntoIterator<Item = Out>,
    I::IntoIter: Send + 'static, {
    Self::from_iterator(values)
  }

  /// Creates a source from an output-stream compatible iterator.
  #[must_use]
  pub fn from_output_stream<I>(values: I) -> Self
  where
    I: IntoIterator<Item = Out>,
    I::IntoIter: Send + 'static, {
    Self::from_iterator(values)
  }

  /// Creates a source that emits one value from a future when it becomes ready.
  #[must_use]
  pub fn future<Fut>(future: Fut) -> Self
  where
    Fut: Future<Output = Out> + Send + 'static, {
    Self::from_logic(StageKind::Custom, FutureSourceLogic::<Out, Fut> {
      future: Some(Box::pin(future)),
      done:   false,
      _pd:    PhantomData,
    })
  }

  /// Alias of [`Source::future`].
  #[must_use]
  pub fn future_source<Fut>(future: Fut) -> Self
  where
    Fut: Future<Output = Out> + Send + 'static, {
    Self::future(future)
  }

  /// Alias of [`Source::future`].
  #[must_use]
  pub fn completion_stage<Fut>(future: Fut) -> Self
  where
    Fut: Future<Output = Out> + Send + 'static, {
    Self::future(future)
  }

  /// Alias of [`Source::future`].
  #[must_use]
  pub fn completion_stage_source<Fut>(future: Fut) -> Self
  where
    Fut: Future<Output = Out> + Send + 'static, {
    Self::future(future)
  }

  /// Lazily creates a source from a future factory.
  #[must_use]
  pub fn lazy_future<F, Fut>(factory: F) -> Self
  where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Out> + Send + 'static, {
    Self::future(factory())
  }

  /// Alias of [`Source::lazy_future`].
  #[must_use]
  pub fn lazy_future_source<F, Fut>(factory: F) -> Self
  where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Out> + Send + 'static, {
    Self::lazy_future(factory)
  }

  /// Alias of [`Source::lazy_future`].
  #[must_use]
  pub fn lazy_completion_stage<F, Fut>(factory: F) -> Self
  where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Out> + Send + 'static, {
    Self::lazy_future(factory)
  }

  /// Alias of [`Source::lazy_future`].
  #[must_use]
  pub fn lazy_completion_stage_source<F, Fut>(factory: F) -> Self
  where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Out> + Send + 'static, {
    Self::lazy_future(factory)
  }

  /// Lazily creates a single-element source.
  ///
  /// The factory is deferred until the first element is demanded.
  #[must_use]
  pub fn lazy_single<F>(factory: F) -> Self
  where
    F: FnOnce() -> Out + Send + 'static,
    Out: Sync, {
    Self::lazy_source(move || Self::single(factory()))
  }

  /// Lazily creates a source from a source factory.
  ///
  /// The factory is not called until the first element is demanded.
  /// All elements from the created source are collected and buffered on first pull.
  #[must_use]
  pub fn lazy_source<F>(factory: F) -> Self
  where
    F: FnOnce() -> Self + Send + 'static,
    Out: Sync, {
    Self::from_logic(StageKind::Custom, LazySourceLogic::<Out, F> {
      factory: Some(factory),
      buffer:  VecDeque::new(),
      error:   None,
      _pd:     PhantomData,
    })
  }

  /// Creates an optional source.
  #[must_use]
  pub fn maybe(value: Option<Out>) -> Self {
    Self::from_option(value)
  }

  /// Creates a source materialized as a bounded source queue.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `capacity` is zero.
  pub fn queue(capacity: usize) -> Result<Source<Out, BoundedSourceQueue<Out>>, StreamDslError> {
    let capacity = validate_positive_argument("capacity", capacity)?;
    let queue = BoundedSourceQueue::new(capacity, OverflowStrategy::Backpressure);
    let mut graph = StreamGraph::new();
    let outlet: Outlet<Out> = Outlet::new();
    let logic = QueueSourceLogic::<Out> { queue: queue.clone() };
    let definition = SourceDefinition {
      kind:        StageKind::Custom,
      outlet:      outlet.id(),
      output_type: TypeId::of::<Out>(),
      mat_combine: MatCombine::Right,
      supervision: SupervisionStrategy::Stop,
      restart:     None,
      logic:       Box::new(logic),
      attributes:  Attributes::new(),
    };
    graph.push_stage(StageDefinition::Source(definition));
    Ok(Source { graph, mat: queue, _pd: PhantomData })
  }

  /// Creates a source materialized as a source queue with completion notifications.
  ///
  /// `capacity` may be zero to disable the internal buffer.
  ///
  /// # Errors
  ///
  /// This constructor currently does not fail.
  pub fn queue_with_overflow(
    capacity: usize,
    overflow_strategy: OverflowStrategy,
  ) -> Result<Source<Out, SourceQueueWithComplete<Out>>, StreamDslError> {
    Self::queue_with_overflow_and_max_concurrent_offers(capacity, overflow_strategy, 1)
  }

  /// Creates a source materialized as a source queue with completion notifications.
  ///
  /// `capacity` may be zero to disable the internal buffer.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `max_concurrent_offers` is zero.
  pub fn queue_with_overflow_and_max_concurrent_offers(
    capacity: usize,
    overflow_strategy: OverflowStrategy,
    max_concurrent_offers: usize,
  ) -> Result<Source<Out, SourceQueueWithComplete<Out>>, StreamDslError> {
    let max_concurrent_offers = validate_positive_argument("max_concurrent_offers", max_concurrent_offers)?;
    let queue = SourceQueueWithComplete::new(capacity, overflow_strategy, max_concurrent_offers);
    let mut graph = StreamGraph::new();
    let outlet: Outlet<Out> = Outlet::new();
    let logic = QueueWithOverflowSourceLogic::<Out> { queue: queue.clone() };
    let definition = SourceDefinition {
      kind:        StageKind::Custom,
      outlet:      outlet.id(),
      output_type: TypeId::of::<Out>(),
      mat_combine: MatCombine::Right,
      supervision: SupervisionStrategy::Stop,
      restart:     None,
      logic:       Box::new(logic),
      attributes:  Attributes::new(),
    };
    graph.push_stage(StageDefinition::Source(definition));
    Ok(Source { graph, mat: queue, _pd: PhantomData })
  }

  /// Creates a source materialized as an unbounded source queue.
  #[must_use]
  pub fn queue_unbounded() -> Source<Out, SourceQueue<Out>> {
    let queue = SourceQueue::new();
    let mut graph = StreamGraph::new();
    let outlet: Outlet<Out> = Outlet::new();
    let logic = UnboundedQueueSourceLogic::<Out> { queue: queue.clone() };
    let definition = SourceDefinition {
      kind:        StageKind::Custom,
      outlet:      outlet.id(),
      output_type: TypeId::of::<Out>(),
      mat_combine: MatCombine::Right,
      supervision: SupervisionStrategy::Stop,
      restart:     None,
      logic:       Box::new(logic),
      attributes:  Attributes::new(),
    };
    graph.push_stage(StageDefinition::Source(definition));
    Source { graph, mat: queue, _pd: PhantomData }
  }

  /// Creates a ticking source by repeating and delaying values.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `interval_ticks` is zero.
  pub fn tick(initial_delay_ticks: usize, interval_ticks: usize, value: Out) -> Result<Self, StreamDslError>
  where
    Out: Clone + Sync, {
    validate_positive_argument("interval_ticks", interval_ticks)?;
    let mut source = Self::repeat(value);
    if initial_delay_ticks > 0 {
      source = source.initial_delay(initial_delay_ticks)?;
    }
    Ok(source)
  }

  /// Creates a source by repeatedly unfolding state.
  #[must_use]
  pub fn unfold<S, F>(initial: S, mut func: F) -> Self
  where
    S: Send + 'static,
    F: FnMut(S) -> Option<(S, Out)> + Send + 'static, {
    let mut state = Some(initial);
    Self::from_iterator(core::iter::from_fn(move || {
      let current = state.take()?;
      match func(current) {
        | Some((next, value)) => {
          state = Some(next);
          Some(value)
        },
        | None => None,
      }
    }))
  }

  /// Creates a source by repeatedly unfolding state with an asynchronous function.
  #[must_use]
  pub fn unfold_async<S, F, Fut>(initial: S, func: F) -> Self
  where
    S: Send + 'static,
    F: FnMut(S) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Option<(S, Out)>> + Send + 'static, {
    Self::from_logic(StageKind::Custom, UnfoldAsyncSourceLogic::<S, Out, F, Fut> {
      state: Some(initial),
      func,
      pending: None,
      done: false,
      _pd: PhantomData,
    })
  }

  /// Alias of [`Source::unfold`].
  #[must_use]
  pub fn unfold_resource<S, F>(initial: S, func: F) -> Self
  where
    S: Send + 'static,
    F: FnMut(S) -> Option<(S, Out)> + Send + 'static, {
    Self::unfold(initial, func)
  }

  /// Alias of [`Source::unfold_async`].
  #[must_use]
  pub fn unfold_resource_async<S, F, Fut>(initial: S, func: F) -> Self
  where
    S: Send + 'static,
    F: FnMut(S) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Option<(S, Out)>> + Send + 'static, {
    Self::unfold_async(initial, func)
  }

  /// Alias of [`Source::zip`].
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `n` is zero.
  pub fn zip_n(self, n: usize) -> Result<Source<Vec<Out>, StreamNotUsed>, StreamDslError>
  where
    Out: Sync, {
    self.zip(n)
  }

  /// Alias of [`Source::zip_n`] followed by mapping.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `n` is zero.
  pub fn zip_with_n<T, F>(self, n: usize, func: F) -> Result<Source<T, StreamNotUsed>, StreamDslError>
  where
    Out: Sync,
    T: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> T + Send + Sync + 'static, {
    Ok(self.zip_n(n)?.map(func))
  }

  /// Builds a source directly from custom stage logic.
  pub fn from_logic<L>(kind: StageKind, logic: L) -> Self
  where
    L: SourceLogic + 'static, {
    let mut graph = StreamGraph::new();
    let outlet: Outlet<Out> = Outlet::new();
    let definition = SourceDefinition {
      kind,
      outlet: outlet.id(),
      output_type: TypeId::of::<Out>(),
      mat_combine: MatCombine::Right,
      supervision: SupervisionStrategy::Stop,
      restart: None,
      logic: Box::new(logic),
      attributes: Attributes::new(),
    };
    graph.push_stage(StageDefinition::Source(definition));
    Self { graph, mat: StreamNotUsed::new(), _pd: PhantomData }
  }
}

impl Source<i32, StreamNotUsed> {
  /// Creates a source that emits all integers between `start` and `end` (inclusive).
  #[must_use]
  pub fn range(start: i32, end: i32) -> Self {
    if start <= end {
      return Self::from_iterator(start..=end);
    }
    Self::from_iterator((end..=start).rev())
  }
}

impl<Out, Mat> Source<Out, Mat>
where
  Out: Send + Sync + 'static,
{
  /// Combines two sources using merge fan-in with a custom materialized value
  /// combination rule.
  ///
  /// This is the Pekko `Source.combineMat` equivalent: two sources are merged
  /// and their materialized values are combined via the supplied rule `C`.
  #[must_use]
  pub fn combine_mat<Mat2, C>(first: Source<Out, Mat>, second: Source<Out, Mat2>, _combine: C) -> Source<Out, C::Out>
  where
    C: MatCombineRule<Mat, Mat2>, {
    let (mut graph, left_mat) = first.into_parts();
    let first_outlet = graph.tail_outlet();
    let (second_graph, right_mat) = second.into_parts();
    let second_outlet = second_graph.tail_outlet();
    graph.append_unwired(second_graph);
    let definition = merge_definition::<Out>(2);
    let merge_inlet = definition.inlet;
    graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = first_outlet {
      graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(merge_inlet), MatCombine::Left);
    }
    if let Some(from) = second_outlet {
      graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(merge_inlet), MatCombine::Left);
    }
    let mat = combine_mat::<Mat, Mat2, C>(left_mat, right_mat);
    Source { graph, mat, _pd: PhantomData }
  }

  /// Composes this source with a flow.
  #[must_use]
  pub fn via<T, Mat2>(self, flow: Flow<Out, T, Mat2>) -> Source<T, Mat>
  where
    T: Send + Sync + 'static, {
    self.via_mat(flow, KeepLeft)
  }

  /// Composes this source with a flow using a custom materialized rule.
  #[must_use]
  pub fn via_mat<T, Mat2, C>(self, flow: Flow<Out, T, Mat2>, _combine: C) -> Source<T, C::Out>
  where
    T: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (mut graph, left_mat) = self.into_parts();
    let (flow_graph, right_mat) = flow.into_parts();
    graph.append(flow_graph);
    let mat = combine_mat::<Mat, Mat2, C>(left_mat, right_mat);
    Source { graph, mat, _pd: PhantomData }
  }

  /// Maps this source materialized value.
  #[must_use]
  pub fn map_materialized_value<Mat2, F>(self, func: F) -> Source<Out, Mat2>
  where
    F: FnOnce(Mat) -> Mat2, {
    let (graph, mat) = self.into_parts();
    Source { graph, mat: func(mat), _pd: PhantomData }
  }

  /// Watches stream termination and completes a `StreamCompletion<()>` handle.
  ///
  /// Elements are passed through unchanged. The materialized value is
  /// combined with a fresh `StreamCompletion<()>` using the supplied
  /// `MatCombineRule`.
  #[must_use]
  pub fn watch_termination_mat<C>(mut self, _combine: C) -> Source<Out, C::Out>
  where
    C: MatCombineRule<Mat, StreamCompletion<()>>, {
    let completion = StreamCompletion::<()>::new();
    let definition = watch_termination_definition::<Out>(completion.clone());
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    let mat = combine_mat::<Mat, StreamCompletion<()>, C>(self.mat, completion);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Watches stream termination and discards the completion handle (KeepLeft variant).
  ///
  /// Elements are passed through unchanged. The materialized value of the
  /// upstream source is preserved verbatim.
  #[must_use]
  pub fn watch_termination(self) -> Source<Out, Mat> {
    self.watch_termination_mat(KeepLeft)
  }

  /// Falls back to a secondary source when the primary source emits no elements.
  #[must_use]
  pub fn or_else<Mat2>(mut self, secondary: Source<Out, Mat2>) -> Source<Out, Mat>
  where
    Mat2: Send + Sync + 'static, {
    let definition = or_else_definition::<Out, Mat2>(secondary);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Falls back to a secondary source when the primary source emits no elements
  /// and combines materialized values.
  #[must_use]
  pub fn or_else_mat<Mat2, C>(mut self, secondary: Source<Out, Mat2>, _combine: C) -> Source<Out, C::Out>
  where
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (secondary_graph, right_mat) = secondary.into_parts();
    let definition =
      or_else_definition::<Out, StreamNotUsed>(Source::from_graph(secondary_graph, StreamNotUsed::new()));
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Adds an also-to compatibility stage.
  #[must_use]
  pub fn also_to<Mat2>(self, sink: Sink<Out, Mat2>) -> Source<Out, Mat>
  where
    Out: Clone, {
    self.also_to_mat(sink, KeepLeft)
  }

  /// Adds an also-to stage and combines materialized values.
  #[must_use]
  pub fn also_to_mat<Mat2, C>(self, sink: Sink<Out, Mat2>, _combine: C) -> Source<Out, C::Out>
  where
    Out: Clone,
    C: MatCombineRule<Mat, Mat2>, {
    let (mut graph, left_mat) = self.into_parts();
    let (mut sink_graph, right_mat) = sink.into_parts();
    let broadcast = broadcast_definition::<Out>(2);
    let broadcast_inlet = broadcast.inlet;
    let broadcast_outlet = broadcast.outlet;
    let upstream_outlet = graph.tail_outlet();
    graph.push_stage(StageDefinition::Flow(broadcast));
    if let Some(upstream_outlet) = upstream_outlet {
      graph.connect_or_panic(
        &Outlet::<Out>::from_id(upstream_outlet),
        &Inlet::<Out>::from_id(broadcast_inlet),
        MatCombine::Left,
      );
    }
    let passthrough = map_definition::<Out, Out, _>(|value| value);
    let passthrough_inlet = passthrough.inlet;
    sink_graph.push_stage(StageDefinition::Flow(passthrough));
    graph.append(sink_graph);
    graph.connect_or_panic(
      &Outlet::<Out>::from_id(broadcast_outlet),
      &Inlet::<Out>::from_id(passthrough_inlet),
      MatCombine::Left,
    );
    let mat = combine_mat::<Mat, Mat2, C>(left_mat, right_mat);
    Source::from_graph(graph, mat)
  }

  /// Attaches multiple sinks so that every element is broadcast to all of them
  /// while continuing downstream.
  ///
  /// Uses a single `Broadcast(N+1)` stage where N is the number of sinks and
  /// +1 is the downstream main path.  This mirrors `alsoToAll` in Apache Pekko
  /// (`Flow.scala:3996`) and delivers exactly one clone to each sink and one to
  /// the downstream consumer (linear clone cost), unlike chaining `also_to` N
  /// times which would produce quadratic clones via nested `Broadcast(2)` stages.
  ///
  /// An empty iterator leaves the source unchanged.
  #[must_use]
  pub fn also_to_all<Mat2, I>(self, sinks: I) -> Source<Out, Mat>
  where
    Out: Clone,
    I: IntoIterator<Item = Sink<Out, Mat2>>, {
    // 単一の Broadcast(N+1) を使い、各 sink と downstream の本流に 1 回ずつ clone する。
    // これにより clone 回数が O(N) になり、fold による Broadcast(2) の縦積み（O(N^2)）を避ける。
    let sinks: Vec<Sink<Out, Mat2>> = sinks.into_iter().collect();
    if sinks.is_empty() {
      // sink が 0 本の場合は no-op
      return self;
    }
    let n = sinks.len();
    let fan_out = n + 1; // N sinks + 1 downstream passthrough
    let (mut graph, mat) = self.into_parts();
    let broadcast = broadcast_definition::<Out>(fan_out);
    let broadcast_inlet = broadcast.inlet;
    let broadcast_outlet = broadcast.outlet;
    let upstream_outlet = graph.tail_outlet();
    graph.push_stage(StageDefinition::Flow(broadcast));
    // 上流の outlet を broadcast の inlet に接続する
    if let Some(upstream_outlet) = upstream_outlet {
      graph.connect_or_panic(
        &Outlet::<Out>::from_id(upstream_outlet),
        &Inlet::<Out>::from_id(broadcast_inlet),
        MatCombine::Left,
      );
    }
    // 各 sink の graph を unwired で追加し、broadcast outlet から sink 入口へ接続する
    for sink in sinks {
      let (sink_graph, _sink_mat) = sink.into_parts();
      if let Some(sink_head_inlet) = sink_graph.head_inlet() {
        graph.append_unwired(sink_graph);
        graph.connect_or_panic(
          &Outlet::<Out>::from_id(broadcast_outlet),
          &Inlet::<Out>::from_id(sink_head_inlet),
          MatCombine::Left,
        );
      }
    }
    // 本流（downstream）継続用の passthrough を末尾に追加して broadcast の N+1 本目に接続する
    let passthrough = map_definition::<Out, Out, _>(|value| value);
    let passthrough_inlet = passthrough.inlet;
    graph.push_stage(StageDefinition::Flow(passthrough));
    graph.connect_or_panic(
      &Outlet::<Out>::from_id(broadcast_outlet),
      &Inlet::<Out>::from_id(passthrough_inlet),
      MatCombine::Left,
    );
    Source::from_graph(graph, mat)
  }

  /// Adds a divert-to stage that routes elements matching the predicate to a sink.
  ///
  /// Elements matching `predicate` are sent to `sink`; non-matching elements
  /// continue downstream.
  #[must_use]
  pub fn divert_to<Mat2, F>(self, predicate: F, sink: Sink<Out, Mat2>) -> Source<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    self.divert_to_mat(predicate, sink, KeepLeft)
  }

  /// Adds a divert-to stage and combines materialized values.
  ///
  /// Elements matching `predicate` are sent to `sink`; non-matching elements
  /// continue downstream.
  #[must_use]
  pub fn divert_to_mat<Mat2, F, C>(self, predicate: F, sink: Sink<Out, Mat2>, _combine: C) -> Source<Out, C::Out>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (mut graph, left_mat) = self.into_parts();
    let (mut sink_graph, right_mat) = sink.into_parts();
    let partition = partition_definition::<Out, F>(predicate);
    let partition_inlet = partition.inlet;
    let partition_outlet = partition.outlet;
    let upstream_outlet = graph.tail_outlet();
    graph.push_stage(StageDefinition::Flow(partition));
    if let Some(upstream_outlet) = upstream_outlet {
      graph.connect_or_panic(
        &Outlet::<Out>::from_id(upstream_outlet),
        &Inlet::<Out>::from_id(partition_inlet),
        MatCombine::Left,
      );
    }
    let passthrough = map_definition::<Out, Out, _>(|value| value);
    let passthrough_inlet = passthrough.inlet;
    sink_graph.push_stage(StageDefinition::Flow(passthrough));
    graph.append(sink_graph);
    graph.connect_or_panic(
      &Outlet::<Out>::from_id(partition_outlet),
      &Inlet::<Out>::from_id(passthrough_inlet),
      MatCombine::Left,
    );
    let mat = combine_mat::<Mat, Mat2, C>(left_mat, right_mat);
    Source::from_graph(graph, mat)
  }

  /// Connects this source to a sink.
  #[must_use]
  pub fn to<Mat2>(self, sink: Sink<Out, Mat2>) -> RunnableGraph<Mat> {
    self.into_mat(sink, KeepLeft)
  }

  /// Connects this source to a sink using a custom materialized rule.
  ///
  /// # Panics
  ///
  /// Panics when the stream graph cannot be converted into a runnable plan.
  #[must_use]
  pub fn into_mat<Mat2, C>(self, sink: Sink<Out, Mat2>, _combine: C) -> RunnableGraph<C::Out>
  where
    C: MatCombineRule<Mat, Mat2>, {
    let (mut graph, left_mat) = self.into_parts();
    let (sink_graph, right_mat) = sink.into_parts();
    graph.append(sink_graph);
    let mat = combine_mat::<Mat, Mat2, C>(left_mat, right_mat);
    let plan = match graph.into_plan() {
      | Ok(plan) => plan,
      | Err(error) => panic!("invalid stream graph: {error}"),
    };
    RunnableGraph::new(plan, mat)
  }

  /// Runs this source with the provided sink and materializer.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when materialization fails.
  pub fn run_with<Mat2, M>(
    self,
    sink: Sink<Out, Mat2>,
    materializer: &mut M,
  ) -> Result<Materialized<Mat2>, StreamError>
  where
    M: Materializer, {
    self.into_mat(sink, KeepRight).run(materializer)
  }

  /// Runs this source with a folding sink shortcut.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when materialization fails.
  pub fn run_fold<Acc, F, M>(
    self,
    initial: Acc,
    func: F,
    materializer: &mut M,
  ) -> Result<Materialized<StreamCompletion<Acc>>, StreamError>
  where
    Acc: Send + Sync + 'static,
    F: FnMut(Acc, Out) -> Acc + Send + Sync + 'static,
    M: Materializer, {
    self.run_with(Sink::fold(initial, func), materializer)
  }

  /// Runs this source with an async-fold sink shortcut.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when materialization fails.
  pub fn run_fold_async<Acc, F, Fut, M>(
    self,
    initial: Acc,
    func: F,
    materializer: &mut M,
  ) -> Result<Materialized<StreamCompletion<Acc>>, StreamError>
  where
    Acc: Clone + Send + Sync + 'static,
    F: FnMut(Acc, Out) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Acc> + Send + 'static,
    M: Materializer, {
    self.run_with(Sink::fold_async(initial, func), materializer)
  }

  /// Runs this source with a reducing sink shortcut.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when materialization fails.
  pub fn run_reduce<F, M>(
    self,
    func: F,
    materializer: &mut M,
  ) -> Result<Materialized<StreamCompletion<Out>>, StreamError>
  where
    F: FnMut(Out, Out) -> Out + Send + Sync + 'static,
    M: Materializer, {
    self.run_with(Sink::reduce(func), materializer)
  }

  /// Runs this source with a foreach sink shortcut.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when materialization fails.
  pub fn run_foreach<F, M>(
    self,
    func: F,
    materializer: &mut M,
  ) -> Result<Materialized<StreamCompletion<StreamDone>>, StreamError>
  where
    F: FnMut(Out) + Send + Sync + 'static,
    M: Materializer, {
    self.run_with(Sink::foreach(func), materializer)
  }

  /// Adds a map stage to this source.
  #[must_use]
  pub fn map<T, F>(mut self, func: F) -> Source<T, Mat>
  where
    T: Send + 'static,
    F: FnMut(Out) -> T + Send + Sync + 'static, {
    let definition = map_definition::<Out, T, F>(func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds an async map stage to this source.
  ///
  /// This is a compatibility entry point for Pekko's `map_async`.
  /// `parallelism` is validated as a positive integer.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  #[must_use = "resulting source should be used for further stream composition"]
  pub fn map_async<T, F, Fut>(mut self, parallelism: usize, func: F) -> Result<Source<T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static, {
    let parallelism = validate_positive_argument("parallelism", parallelism)?;
    let definition = map_async_definition::<Out, T, F, Fut>(parallelism, func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a stateful-map stage to this source.
  #[must_use]
  pub fn stateful_map<T, Factory, Mapper>(mut self, factory: Factory) -> Source<T, Mat>
  where
    T: Send + Sync + 'static,
    Factory: FnMut() -> Mapper + Send + Sync + 'static,
    Mapper: FnMut(Out) -> T + Send + Sync + 'static, {
    let definition = stateful_map_definition::<Out, T, Factory, Mapper>(factory);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a stateful-map-concat stage to this source.
  #[must_use]
  pub fn stateful_map_concat<T, Factory, Mapper, I>(mut self, factory: Factory) -> Source<T, Mat>
  where
    T: Send + Sync + 'static,
    Factory: FnMut() -> Mapper + Send + Sync + 'static,
    Mapper: FnMut(Out) -> I + Send + Sync + 'static,
    I: IntoIterator<Item = T> + 'static, {
    let definition = stateful_map_concat_definition::<Out, T, Factory, Mapper, I>(factory);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a stateful-map stage with an on-complete callback to this source.
  ///
  /// The `factory` closure creates the initial state, `mapper` transforms
  /// each element with mutable access to the state, and `on_complete` is
  /// called once when the upstream completes, optionally emitting a final
  /// element.
  #[must_use]
  pub fn stateful_map_with_on_complete<T, S, Factory, Mapper, OnComplete>(
    mut self,
    factory: Factory,
    mapper: Mapper,
    on_complete: OnComplete,
  ) -> Source<T, Mat>
  where
    T: Send + Sync + 'static,
    S: Send + Sync + 'static,
    Factory: FnMut() -> S + Send + Sync + 'static,
    Mapper: FnMut(&mut S, Out) -> T + Send + Sync + 'static,
    OnComplete: FnMut(S) -> Option<T> + Send + Sync + 'static, {
    let definition =
      stateful_map_with_on_complete_definition::<Out, T, S, Factory, Mapper, OnComplete>(factory, mapper, on_complete);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a stateful-map-concat stage using a [`StatefulMapConcatAccumulator`] to this source.
  ///
  /// The `factory` closure creates a fresh accumulator for each
  /// materialization. [`StatefulMapConcatAccumulator::on_complete`] is
  /// called when the upstream completes, allowing trailing elements to be
  /// emitted.
  ///
  /// [`StatefulMapConcatAccumulator`]: crate::core::dsl::StatefulMapConcatAccumulator
  #[must_use]
  pub fn stateful_map_concat_with_accumulator<T, Factory, Acc>(mut self, factory: Factory) -> Source<T, Mat>
  where
    T: Send + Sync + 'static,
    Factory: FnMut() -> Acc + Send + Sync + 'static,
    Acc: StatefulMapConcatAccumulator<Out, T> + 'static, {
    let definition = stateful_map_concat_accumulator_definition::<Out, T, Factory, Acc>(factory);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a map-concat stage to this source.
  #[must_use]
  pub fn map_concat<T, F, I>(mut self, func: F) -> Source<T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> I + Send + Sync + 'static,
    I: IntoIterator<Item = T> + 'static, {
    let definition = map_concat_definition::<Out, T, F, I>(func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a map-option stage to this source.
  #[must_use]
  pub fn map_option<T, F>(mut self, func: F) -> Source<T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> Option<T> + Send + Sync + 'static, {
    let definition = map_option_definition::<Out, T, F>(func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Collects only values mapped to `Some`.
  #[must_use]
  pub fn collect<T, F>(self, func: F) -> Source<T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> Option<T> + Send + Sync + 'static, {
    self.map_option(func)
  }

  /// Adds a filter stage to this source.
  #[must_use]
  pub fn filter<F>(mut self, predicate: F) -> Source<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = filter_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a filter-not stage to this source.
  #[must_use]
  pub fn filter_not<F>(self, mut predicate: F) -> Source<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    self.filter(move |value| !predicate(value))
  }

  /// Adds a drop stage that skips the first `count` elements.
  #[must_use]
  pub fn drop(mut self, count: usize) -> Source<Out, Mat> {
    let definition = drop_definition::<Out>(count);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a take stage that emits up to `count` elements.
  #[must_use]
  pub fn take(mut self, count: usize) -> Source<Out, Mat> {
    let definition = take_definition::<Out>(count);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a drop-while stage to this source.
  #[must_use]
  pub fn drop_while<F>(mut self, predicate: F) -> Source<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = drop_while_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a take-while stage to this source.
  #[must_use]
  pub fn take_while<F>(mut self, predicate: F) -> Source<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = take_while_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a take-until stage to this source.
  #[must_use]
  pub fn take_until<F>(mut self, predicate: F) -> Source<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = take_until_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a grouped stage that emits vectors of size `size`.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` is zero.
  pub fn grouped(mut self, size: usize) -> Result<Source<Vec<Out>, Mat>, StreamDslError> {
    let size = validate_positive_argument("size", size)?;
    let definition = grouped_definition::<Out>(size);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a sliding stage that emits windows with size `size`.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` is zero.
  pub fn sliding(mut self, size: usize) -> Result<Source<Vec<Out>, Mat>, StreamDslError>
  where
    Out: Clone, {
    let size = validate_positive_argument("size", size)?;
    let definition = sliding_definition::<Out>(size);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a scan stage that emits running accumulation from `initial`.
  #[must_use]
  pub fn scan<Acc, F>(mut self, initial: Acc, func: F) -> Source<Acc, Mat>
  where
    Acc: Clone + Send + Sync + 'static,
    F: FnMut(Acc, Out) -> Acc + Send + Sync + 'static, {
    let definition = scan_definition::<Out, Acc, F>(initial, func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a fold stage that emits running accumulation (excluding the initial value).
  ///
  /// Equivalent to `scan(initial, func).drop(1)`.
  #[must_use]
  pub fn fold<Acc, F>(self, initial: Acc, func: F) -> Source<Acc, Mat>
  where
    Acc: Clone + Send + Sync + 'static,
    F: FnMut(Acc, Out) -> Acc + Send + Sync + 'static, {
    self.scan(initial, func).drop(1)
  }

  /// Adds a reduce stage that folds elements using the first element as the seed,
  /// emitting the running reduction for each subsequent element.
  #[must_use]
  pub fn reduce<F>(self, mut func: F) -> Source<Out, Mat>
  where
    Out: Clone,
    F: FnMut(Out, Out) -> Out + Send + Sync + 'static, {
    self
      .scan(None::<Out>, move |acc, value| {
        Some(match acc {
          | Some(current) => (func)(current, value),
          | None => value,
        })
      })
      .drop(1)
      .flatten_optional()
  }

  /// Adds an intersperse stage with start, separator and end markers.
  #[must_use]
  pub fn intersperse(mut self, start: Out, inject: Out, end: Out) -> Source<Out, Mat>
  where
    Out: Clone, {
    let definition = intersperse_definition::<Out>(start, inject, end);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a flatMapConcat stage to this source.
  #[must_use]
  pub fn flat_map_concat<T, Mat2, F>(mut self, func: F) -> Source<T, Mat>
  where
    T: Send + Sync + 'static,
    Mat2: Send + Sync + 'static,
    F: FnMut(Out) -> Source<T, Mat2> + Send + Sync + 'static, {
    let definition = flat_map_concat_definition::<Out, T, Mat2, F>(func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a flatMapMerge stage to this source.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `breadth` is zero.
  pub fn flat_map_merge<T, Mat2, F>(mut self, breadth: usize, func: F) -> Result<Source<T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    Mat2: Send + Sync + 'static,
    F: FnMut(Out) -> Source<T, Mat2> + Send + Sync + 'static, {
    let breadth = validate_positive_argument("breadth", breadth)?;
    let definition = flat_map_merge_definition::<Out, T, Mat2, F>(breadth, func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a buffer stage with an overflow strategy.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `capacity` is zero.
  pub fn buffer(
    mut self,
    capacity: usize,
    overflow_strategy: OverflowStrategy,
  ) -> Result<Source<Out, Mat>, StreamDslError> {
    let capacity = validate_positive_argument("capacity", capacity)?;
    let definition = buffer_definition::<Out>(capacity, overflow_strategy);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Marks this source with an async boundary attribute.
  ///
  /// The materializer uses this attribute to split the graph into
  /// independently executed islands. The boundary is resolved at
  /// materialization time and does not insert a buffer stage.
  ///
  /// Mirrors Pekko's `Graph.async`.
  #[must_use]
  pub fn r#async(mut self) -> Source<Out, Mat> {
    self.graph.mark_last_node_async();
    self
  }

  /// Marks this source with an async boundary attribute and a named dispatcher.
  ///
  /// The dispatcher is attached to the island downstream of this
  /// boundary. During materialization, that downstream island actor is
  /// spawned with the specified dispatcher as its execution context.
  #[must_use]
  pub fn async_with_dispatcher(mut self, dispatcher: impl Into<String>) -> Source<Out, Mat> {
    self.graph.mark_last_node_async();
    self.graph.mark_last_node_dispatcher(dispatcher);
    self
  }

  /// Adds a throttle stage that limits the number of buffered in-flight elements.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `capacity` is zero.
  pub fn throttle(mut self, capacity: usize, mode: ThrottleMode) -> Result<Source<Out, Mat>, StreamDslError> {
    let capacity = validate_positive_argument("capacity", capacity)?;
    let definition = throttle_definition::<Out>(capacity, mode);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a delay stage that emits each element after `ticks`.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn delay(mut self, ticks: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = delay_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds an initial-delay stage that suppresses outputs until `ticks` elapse.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn initial_delay(mut self, ticks: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = initial_delay_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a take-within stage that forwards elements only within `ticks`.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn take_within(mut self, ticks: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = take_within_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a debounce stage that emits the held element after `ticks` of silence.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn debounce(mut self, ticks: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = debounce_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a sample stage that emits the latest element at fixed `ticks` intervals.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn sample(mut self, ticks: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = sample_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a batch stage that emits vectors of size `size`.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` is zero.
  pub fn batch(mut self, size: usize) -> Result<Source<Vec<Out>, Mat>, StreamDslError> {
    let size = validate_positive_argument("size", size)?;
    let definition = batch_definition::<Out>(size);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Aggregates elements with boundary semantics (alias for [`Self::batch`]).
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` is zero.
  pub fn aggregate_with_boundary(self, size: usize) -> Result<Source<Vec<Out>, Mat>, StreamDslError> {
    self.batch(size)
  }

  /// Batches elements with weighted semantics.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `max_weight` is zero.
  pub fn batch_weighted<FW>(
    mut self,
    max_weight: usize,
    weight_fn: FW,
  ) -> Result<Source<Vec<Out>, Mat>, StreamDslError>
  where
    FW: FnMut(&Out) -> usize + Send + Sync + 'static, {
    let max_weight = validate_positive_argument("max_weight", max_weight)?;
    let definition = grouped_weighted_definition::<Out, FW>(max_weight, weight_fn);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Conflates upstream elements by repeatedly aggregating all emitted values.
  #[must_use]
  pub fn conflate<FA>(self, aggregate: FA) -> Source<Out, Mat>
  where
    Out: Send + Sync + 'static,
    FA: FnMut(Out, Out) -> Out + Send + Sync + 'static, {
    self.conflate_with_seed(|value| value, aggregate)
  }

  /// Adds a conflate-with-seed stage.
  #[must_use]
  pub fn conflate_with_seed<T, FS, FA>(mut self, seed: FS, aggregate: FA) -> Source<T, Mat>
  where
    Out: 'static,
    T: Send + Sync + 'static,
    FS: FnMut(Out) -> T + Send + Sync + 'static,
    FA: FnMut(T, Out) -> T + Send + Sync + 'static, {
    let definition = conflate_with_seed_definition::<Out, T, FS, FA>(seed, aggregate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Expands each input element and extrapolates on idle ticks while upstream is active.
  #[must_use]
  pub fn expand<F, I>(mut self, expander: F) -> Source<Out, Mat>
  where
    F: FnMut(&Out) -> I + Send + Sync + 'static,
    I: IntoIterator<Item = Out> + 'static,
    <I as IntoIterator>::IntoIter: Send, {
    let definition = expand_definition::<Out, F, I>(expander);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Extrapolates elements with the same behavior as [`Self::expand`].
  #[must_use]
  pub fn extrapolate<F, I>(self, expander: F) -> Source<Out, Mat>
  where
    F: FnMut(&Out) -> I + Send + Sync + 'static,
    I: IntoIterator<Item = Out> + 'static,
    <I as IntoIterator>::IntoIter: Send, {
    self.expand(expander)
  }

  /// Fails the stream when downstream backpressure exceeds `ticks`.
  ///
  /// Mirrors [`Flow::backpressure_timeout`] on the Source DSL. After the first element arrives, if
  /// no subsequent `apply` call occurs within `ticks` ticks, the stream fails with
  /// [`StreamError::Timeout`].
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn backpressure_timeout(mut self, ticks: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = backpressure_timeout_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Fails the stream when it does not complete within `ticks`.
  ///
  /// Mirrors [`Flow::completion_timeout`] on the Source DSL. The tick counter starts at stream
  /// start. If the stream has not completed by the time `tick_count` exceeds `ticks`, the stream
  /// fails with [`StreamError::Timeout`].
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn completion_timeout(mut self, ticks: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = completion_timeout_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Fails the stream when no element arrives within `ticks`.
  ///
  /// Mirrors [`Flow::idle_timeout`] on the Source DSL. The tick counter starts at stream start and
  /// resets on every element. If the gap between successive elements (or between start and the
  /// first element) exceeds `ticks`, the stream fails with [`StreamError::Timeout`].
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn idle_timeout(mut self, ticks: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = idle_timeout_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Fails the stream when the first element does not arrive within `ticks`.
  ///
  /// Mirrors [`Flow::initial_timeout`] on the Source DSL. If `tick_count` exceeds `ticks` before
  /// the first `apply` call, the stream fails with [`StreamError::Timeout`]. Once the first element
  /// arrives, this stage becomes a pure pass-through.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn initial_timeout(mut self, ticks: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = initial_timeout_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Injects a keep-alive element when no element arrives within `ticks`.
  ///
  /// Mirrors [`Flow::keep_alive`] on the Source DSL.
  /// This mirrors `keepAlive` in Apache Pekko (`Flow.scala:3080`).
  /// When the upstream is idle for `ticks` ticks, `value` is injected downstream.
  /// Normal elements pass through unchanged and reset the idle timer.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn keep_alive(mut self, ticks: usize, value: Out) -> Result<Source<Out, Mat>, StreamDslError>
  where
    Out: Clone, {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = keep_alive_definition::<Out>(ticks as u64, value);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a wire-tap compatibility stage that observes each element without altering the data
  /// path.
  ///
  /// Mirrors [`Flow::wire_tap`] on the Source DSL. The `callback` is invoked for each element
  /// before it continues downstream; the data path is unaffected.
  #[must_use]
  pub fn wire_tap<F>(self, mut callback: F) -> Source<Out, Mat>
  where
    F: FnMut(&Out) + Send + Sync + 'static, {
    self.map(move |value| {
      callback(&value);
      value
    })
  }

  /// Adds a monitor compatibility stage that pairs each element with its zero-based index.
  ///
  /// Mirrors [`Flow::monitor`] on the Source DSL. Each element is emitted as `(index, value)`.
  #[must_use]
  pub fn monitor(self) -> Source<(u64, Out), Mat> {
    self.zip_with_index().map(|(value, index)| (index, value))
  }

  /// Adds a logging stage and metadata while passing each element through unchanged.
  ///
  /// Mirrors [`Flow::log`] on the Source DSL.
  #[must_use]
  pub fn log(mut self, name: &'static str) -> Source<Out, Mat> {
    let definition = log_definition::<Out>();
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }.add_attributes(Attributes::named(name))
  }

  /// Adds a marker-tagged logging stage and marker metadata while passing each element through
  /// unchanged.
  ///
  /// Mirrors [`Flow::log_with_marker`] on the Source DSL.
  #[must_use]
  pub fn log_with_marker(self, name: &'static str, marker: &'static str) -> Source<Out, Mat> {
    self.log(name).add_attributes(Attributes::named(marker))
  }

  /// Cancels the previous inner source and starts a new one for each outer element.
  ///
  /// Mirrors [`Flow::switch_map`] on the Source DSL.
  /// This mirrors `switchMap` in Apache Pekko (`Flow.scala:3002`).
  /// Unlike `flat_map_concat` / `flat_map_merge(1, …)`, which wait for the current
  /// inner source to finish, `switch_map` **immediately** discards the in-progress
  /// inner source when a new outer element arrives.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when switch-map configuration is invalid.
  pub fn switch_map<T, Mat2, F>(mut self, func: F) -> Result<Source<T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    Mat2: Send + Sync + 'static,
    F: FnMut(Out) -> Source<T, Mat2> + Send + Sync + 'static, {
    let definition = switch_map_definition::<Out, T, Mat2, F>(func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a merge-latest stage that emits a `Vec<Out>` snapshot whenever any input is updated.
  ///
  /// Mirrors [`Flow::merge_latest`] on the Source DSL. No output is produced until every input has
  /// delivered at least one element.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn merge_latest(mut self, fan_in: usize) -> Result<Source<Vec<Out>, Mat>, StreamDslError>
  where
    Out: Clone, {
    validate_positive_argument("fan_in", fan_in)?;
    let definition = merge_latest_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a merge-preferred stage that prioritizes slot 0 (preferred) input.
  ///
  /// Mirrors [`Flow::merge_preferred`] on the Source DSL.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn merge_preferred(mut self, fan_in: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    validate_positive_argument("fan_in", fan_in)?;
    let definition = merge_preferred_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Enables restart semantics with backoff for this source.
  #[must_use]
  pub fn restart_source_with_backoff(mut self, min_backoff_ticks: u32, max_restarts: usize) -> Source<Out, Mat> {
    self.graph.set_source_restart(&Some(RestartBackoff::new(min_backoff_ticks, max_restarts)));
    self
  }

  /// Compatibility alias for applying restart-on-failure backoff semantics.
  #[must_use]
  pub fn on_failures_with_backoff(self, min_backoff_ticks: u32, max_restarts: usize) -> Source<Out, Mat> {
    self.restart_source_with_backoff(min_backoff_ticks, max_restarts)
  }

  /// Compatibility alias for applying restart backoff semantics.
  #[must_use]
  pub fn with_backoff(self, min_backoff_ticks: u32, max_restarts: usize) -> Source<Out, Mat> {
    self.restart_source_with_backoff(min_backoff_ticks, max_restarts)
  }

  /// Compatibility alias for applying restart backoff semantics with ignored context parameter.
  #[must_use]
  pub fn with_backoff_and_context<C>(
    self,
    min_backoff_ticks: u32,
    max_restarts: usize,
    _context: C,
  ) -> Source<Out, Mat> {
    self.restart_source_with_backoff(min_backoff_ticks, max_restarts)
  }

  /// Enables restart semantics by explicit restart settings.
  #[must_use]
  pub fn restart_source_with_settings(mut self, settings: RestartConfig) -> Source<Out, Mat> {
    self.graph.set_source_restart(&Some(RestartBackoff::from_settings(settings)));
    self
  }

  /// Applies stop supervision semantics to this source.
  #[must_use]
  pub fn supervision_stop(mut self) -> Source<Out, Mat> {
    self.graph.set_source_supervision(SupervisionStrategy::Stop);
    self
  }

  /// Applies resume supervision semantics to this source.
  #[must_use]
  pub fn supervision_resume(mut self) -> Source<Out, Mat> {
    self.graph.set_source_supervision(SupervisionStrategy::Resume);
    self
  }

  /// Applies restart supervision semantics to this source.
  #[must_use]
  pub fn supervision_restart(mut self) -> Source<Out, Mat> {
    self.graph.set_source_supervision(SupervisionStrategy::Restart);
    self
  }

  /// Replaces graph attributes with the provided values.
  #[must_use]
  pub fn with_attributes(mut self, attributes: Attributes) -> Source<Out, Mat> {
    self.graph.set_attributes(attributes);
    self
  }

  /// Appends graph attributes to the existing values.
  #[must_use]
  pub fn add_attributes(mut self, attributes: Attributes) -> Source<Out, Mat> {
    self.graph.add_attributes(attributes);
    self
  }

  /// Assigns a debug name attribute to this stage graph.
  #[must_use]
  pub fn named(self, name: &str) -> Source<Out, Mat> {
    self.add_attributes(Attributes::named(name))
  }

  /// Adds a group-by stage and returns substream surface for merging grouped elements.
  ///
  /// Unsupported `SubFlow` operators, such as `drop`, stay unavailable on the
  /// returned surface.
  ///
  /// ```compile_fail
  /// use fraktor_stream_core_rs::core::{SubstreamCancelStrategy, dsl::Source};
  ///
  /// let _ = Source::single(1_u32)
  ///   .group_by(2, |value: &u32| value % 2, SubstreamCancelStrategy::default())
  ///   .expect("group_by")
  ///   .drop(1);
  /// ```
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `max_substreams` is zero.
  pub fn group_by<Key, F>(
    mut self,
    max_substreams: usize,
    key_fn: F,
    cancel_strategy: SubstreamCancelStrategy,
  ) -> Result<SourceGroupBySubFlow<Key, Out, Mat>, StreamDslError>
  where
    Key: Clone + PartialEq + Send + Sync + 'static,
    F: FnMut(&Out) -> Key + Send + Sync + 'static, {
    let max_substreams = validate_positive_argument("max_substreams", max_substreams)?;
    let definition = group_by_definition::<Out, Key, F>(max_substreams, key_fn, cancel_strategy);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    let grouped = Source::<(Key, Out), Mat> { graph: self.graph, mat: self.mat, _pd: PhantomData };
    Ok(SourceGroupBySubFlow::from_source(grouped))
  }

  /// Splits the stream before elements matching `predicate`.
  #[must_use]
  pub fn split_when<F>(mut self, predicate: F) -> SourceSubFlow<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = split_when_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    SourceSubFlow::from_source(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Splits the stream before elements matching `predicate` with explicit substream cancellation
  /// handling.
  #[must_use]
  pub fn split_when_with_cancel_strategy<F>(
    mut self,
    substream_cancel_strategy: SubstreamCancelStrategy,
    predicate: F,
  ) -> SourceSubFlow<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = split_when_definition_with_cancel_strategy::<Out, F>(predicate, substream_cancel_strategy);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    SourceSubFlow::from_source(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Splits the stream after elements matching `predicate`.
  #[must_use]
  pub fn split_after<F>(mut self, predicate: F) -> SourceSubFlow<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = split_after_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    SourceSubFlow::from_source(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Splits the stream after elements matching `predicate` with explicit substream cancellation
  /// handling.
  #[must_use]
  pub fn split_after_with_cancel_strategy<F>(
    mut self,
    substream_cancel_strategy: SubstreamCancelStrategy,
    predicate: F,
  ) -> SourceSubFlow<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = split_after_definition_with_cancel_strategy::<Out, F>(predicate, substream_cancel_strategy);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    SourceSubFlow::from_source(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a partition stage that routes each element to one of two output lanes.
  #[must_use]
  pub fn partition<F>(mut self, predicate: F) -> Source<Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = partition_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds an unzip-with stage that maps each element into a pair and routes them to two output
  /// lanes.
  #[must_use]
  pub fn unzip_with<T, F>(mut self, func: F) -> Source<T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> (T, T) + Send + Sync + 'static, {
    let definition = unzip_with_definition::<Out, T, F>(func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a broadcast stage that duplicates each element `fan_out` times.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_out` is zero.
  pub fn broadcast(mut self, fan_out: usize) -> Result<Source<Out, Mat>, StreamDslError>
  where
    Out: Clone, {
    validate_positive_argument("fan_out", fan_out)?;
    let definition = broadcast_definition::<Out>(fan_out);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a balance stage that distributes elements across `fan_out` outputs.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_out` is zero.
  pub fn balance(mut self, fan_out: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    validate_positive_argument("fan_out", fan_out)?;
    let definition = balance_definition::<Out>(fan_out);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a merge stage that merges `fan_in` upstream paths.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn merge(mut self, fan_in: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    validate_positive_argument("fan_in", fan_in)?;
    let definition = merge_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds an interleave stage that consumes `fan_in` inputs in round-robin order.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn interleave(mut self, fan_in: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    validate_positive_argument("fan_in", fan_in)?;
    let definition = interleave_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a prepend stage that prioritizes lower-index input lanes.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn prepend(mut self, fan_in: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    validate_positive_argument("fan_in", fan_in)?;
    let definition = prepend_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a zip stage that emits one vector after receiving one element from each input.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn zip(mut self, fan_in: usize) -> Result<Source<Vec<Out>, Mat>, StreamDslError> {
    validate_positive_argument("fan_in", fan_in)?;
    let definition = zip_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a zip-all stage that fills missing lanes with `fill_value` after completion.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn zip_all(mut self, fan_in: usize, fill_value: Out) -> Result<Source<Vec<Out>, Mat>, StreamDslError>
  where
    Out: Clone, {
    validate_positive_argument("fan_in", fan_in)?;
    let definition = zip_all_definition::<Out>(fan_in, fill_value);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a zip-with-index stage that pairs each element with an incrementing index.
  #[must_use]
  pub fn zip_with_index(mut self) -> Source<(Out, u64), Mat> {
    let definition = zip_with_index_definition::<Out>();
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a concat stage that emits all elements from each input in port order.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn concat(mut self, fan_in: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    validate_positive_argument("fan_in", fan_in)?;
    let definition = concat_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Zip stage that combines materialized values.
  #[must_use]
  pub fn zip_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Source<Vec<Out>, C::Out>
  where
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (source_graph, right_mat) = source.into_parts();
    let source_tail = source_graph.tail_outlet();
    let from = self.graph.tail_outlet();
    self.graph.append_unwired(source_graph);
    let definition = zip_definition::<Out>(2);
    let inlet_id = definition.inlet;
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    if let Some(src_out) = source_tail {
      self.graph.connect_or_panic(
        &Outlet::<Out>::from_id(src_out),
        &Inlet::<Out>::from_id(inlet_id),
        MatCombine::Right,
      );
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Zip-all stage that combines materialized values.
  #[must_use]
  pub fn zip_all_mat<Mat2, C>(
    mut self,
    source: Source<Out, Mat2>,
    fill_value: Out,
    _combine: C,
  ) -> Source<Vec<Out>, C::Out>
  where
    Out: Clone,
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (source_graph, right_mat) = source.into_parts();
    let source_tail = source_graph.tail_outlet();
    let from = self.graph.tail_outlet();
    self.graph.append_unwired(source_graph);
    let definition = zip_all_definition::<Out>(2, fill_value);
    let inlet_id = definition.inlet;
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    if let Some(src_out) = source_tail {
      self.graph.connect_or_panic(
        &Outlet::<Out>::from_id(src_out),
        &Inlet::<Out>::from_id(inlet_id),
        MatCombine::Right,
      );
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Zip-with stage that combines materialized values.
  #[must_use]
  pub fn zip_with_mat<T, Mat2, F, C>(self, source: Source<Out, Mat2>, func: F, combine: C) -> Source<T, C::Out>
  where
    T: Send + Sync + 'static,
    Mat2: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> T + Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    self.zip_mat(source, combine).map(func)
  }

  /// Zip-latest stage that combines materialized values.
  #[must_use]
  pub fn zip_latest_mat<Mat2, C>(self, source: Source<Out, Mat2>, combine: C) -> Source<Vec<Out>, C::Out>
  where
    Out: Clone,
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    self.merge_latest_mat(source, combine)
  }

  /// Zip-latest-with stage that combines materialized values.
  #[must_use]
  pub fn zip_latest_with_mat<T, Mat2, F, C>(self, source: Source<Out, Mat2>, func: F, combine: C) -> Source<T, C::Out>
  where
    Out: Clone,
    T: Send + Sync + 'static,
    Mat2: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> T + Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    self.zip_latest_mat(source, combine).map(func)
  }

  /// Merge stage that combines materialized values.
  #[must_use]
  pub fn merge_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Source<Out, C::Out>
  where
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (source_graph, right_mat) = source.into_parts();
    let source_tail = source_graph.tail_outlet();
    let from = self.graph.tail_outlet();
    self.graph.append_unwired(source_graph);
    let definition = merge_definition::<Out>(2);
    let inlet_id = definition.inlet;
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    if let Some(src_out) = source_tail {
      self.graph.connect_or_panic(
        &Outlet::<Out>::from_id(src_out),
        &Inlet::<Out>::from_id(inlet_id),
        MatCombine::Right,
      );
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Merge-latest stage that combines materialized values.
  #[must_use]
  pub fn merge_latest_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Source<Vec<Out>, C::Out>
  where
    Out: Clone,
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (source_graph, right_mat) = source.into_parts();
    let source_tail = source_graph.tail_outlet();
    let from = self.graph.tail_outlet();
    self.graph.append_unwired(source_graph);
    let definition = merge_latest_definition::<Out>(2);
    let inlet_id = definition.inlet;
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    if let Some(src_out) = source_tail {
      self.graph.connect_or_panic(
        &Outlet::<Out>::from_id(src_out),
        &Inlet::<Out>::from_id(inlet_id),
        MatCombine::Right,
      );
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Merge-preferred stage that combines materialized values.
  #[must_use]
  pub fn merge_preferred_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Source<Out, C::Out>
  where
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (source_graph, right_mat) = source.into_parts();
    let source_tail = source_graph.tail_outlet();
    let from = self.graph.tail_outlet();
    self.graph.append_unwired(source_graph);
    let definition = merge_preferred_definition::<Out>(2);
    let inlet_id = definition.inlet;
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    if let Some(src_out) = source_tail {
      self.graph.connect_or_panic(
        &Outlet::<Out>::from_id(src_out),
        &Inlet::<Out>::from_id(inlet_id),
        MatCombine::Right,
      );
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Merge-prioritized stage that combines materialized values.
  #[must_use]
  pub fn merge_prioritized_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Source<Out, C::Out>
  where
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (source_graph, right_mat) = source.into_parts();
    let source_tail = source_graph.tail_outlet();
    let from = self.graph.tail_outlet();
    self.graph.append_unwired(source_graph);
    let equal_priorities: Vec<usize> = alloc::vec![1; 2];
    let definition = merge_prioritized_definition::<Out>(2, &equal_priorities);
    let inlet_id = definition.inlet;
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    if let Some(src_out) = source_tail {
      self.graph.connect_or_panic(
        &Outlet::<Out>::from_id(src_out),
        &Inlet::<Out>::from_id(inlet_id),
        MatCombine::Right,
      );
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Concat stage that combines materialized values.
  #[must_use]
  pub fn concat_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Source<Out, C::Out>
  where
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (source_graph, right_mat) = source.into_parts();
    let definition =
      concat_lazy_definition::<Out, StreamNotUsed>(Source::from_graph(source_graph, StreamNotUsed::new()));
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Prepend stage that combines materialized values.
  #[must_use]
  pub fn prepend_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Source<Out, C::Out>
  where
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (source_graph, right_mat) = source.into_parts();
    let definition =
      prepend_lazy_definition::<Out, StreamNotUsed>(Source::from_graph(source_graph, StreamNotUsed::new()));
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Interleave stage that combines materialized values.
  #[must_use]
  pub fn interleave_mat<Mat2, C>(
    mut self,
    source: Source<Out, Mat2>,
    _segment_size: usize,
    _combine: C,
  ) -> Source<Out, C::Out>
  where
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (source_graph, right_mat) = source.into_parts();
    let source_tail = source_graph.tail_outlet();
    let from = self.graph.tail_outlet();
    self.graph.append_unwired(source_graph);
    let definition = interleave_definition::<Out>(2);
    let inlet_id = definition.inlet;
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    if let Some(src_out) = source_tail {
      self.graph.connect_or_panic(
        &Outlet::<Out>::from_id(src_out),
        &Inlet::<Out>::from_id(inlet_id),
        MatCombine::Right,
      );
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Flat-map-prefix stage that combines materialized values.
  #[must_use]
  pub fn flat_map_prefix_mat<T, Mat2, F, C>(mut self, prefix: usize, mut factory: F, _combine: C) -> Source<T, C::Out>
  where
    T: Send + Sync + 'static,
    Mat2: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> Flow<Out, T, Mat2> + Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    // Probe factory with empty prefix to extract the materialized value for combination.
    let probe = factory(Vec::new());
    let (_probe_graph, right_mat) = probe.into_parts();
    let definition = flat_map_prefix_definition::<Out, T, Mat2, F>(prefix, factory);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Creates a source from a pre-built stream graph and materialized value.
  #[must_use]
  pub(in crate::core) fn from_graph(graph: StreamGraph, mat: Mat) -> Self {
    Self { graph, mat, _pd: PhantomData }
  }

  pub(in crate::core) fn into_parts(self) -> (StreamGraph, Mat) {
    (self.graph, self.mat)
  }
}

impl<Out, Mat> Source<Out, Mat>
where
  Out: Ord + Send + Sync + 'static,
{
  /// Merge-sorted stage that combines materialized values.
  #[must_use]
  pub fn merge_sorted_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Source<Out, C::Out>
  where
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (source_graph, right_mat) = source.into_parts();
    let source_tail = source_graph.tail_outlet();
    let from = self.graph.tail_outlet();
    self.graph.append_unwired(source_graph);
    let definition = merge_sorted_definition::<Out>(2);
    let inlet_id = definition.inlet;
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::Left);
    }
    if let Some(src_out) = source_tail {
      self.graph.connect_or_panic(
        &Outlet::<Out>::from_id(src_out),
        &Inlet::<Out>::from_id(inlet_id),
        MatCombine::Right,
      );
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Source { graph: self.graph, mat, _pd: PhantomData }
  }
}

impl<Out, Mat> Source<(Out, Out), Mat>
where
  Out: Send + Sync + 'static,
{
  /// Adds an unzip stage that routes tuple components to two output lanes.
  #[must_use]
  pub fn unzip(mut self) -> Source<Out, Mat> {
    let definition = unzip_definition::<Out>();
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(
        &Outlet::<(Out, Out)>::from_id(from),
        &Inlet::<(Out, Out)>::from_id(inlet_id),
        MatCombine::Left,
      );
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }
}

impl<Out, Mat> Source<Vec<Out>, Mat>
where
  Out: Send + Sync + 'static,
{
  /// Merges split substreams into a single output stream.
  #[must_use]
  pub fn merge_substreams(mut self) -> Source<Out, Mat> {
    let definition = merge_substreams_definition::<Out>();
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(
        &Outlet::<Vec<Out>>::from_id(from),
        &Inlet::<Vec<Out>>::from_id(inlet_id),
        MatCombine::Left,
      );
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Merges split substreams with an explicit parallelism value.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn merge_substreams_with_parallelism(mut self, parallelism: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    let parallelism = validate_positive_argument("parallelism", parallelism)?;
    let definition = merge_substreams_with_parallelism_definition::<Out>(parallelism);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(
        &Outlet::<Vec<Out>>::from_id(from),
        &Inlet::<Vec<Out>>::from_id(inlet_id),
        MatCombine::Left,
      );
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Concatenates split substreams into a single output stream.
  #[must_use]
  pub fn concat_substreams(mut self) -> Source<Out, Mat> {
    let definition = concat_substreams_definition::<Out>();
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(
        &Outlet::<Vec<Out>>::from_id(from),
        &Inlet::<Vec<Out>>::from_id(inlet_id),
        MatCombine::Left,
      );
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }
}

impl<Out, Mat> Source<Option<Out>, Mat>
where
  Out: Send + Sync + 'static,
{
  /// Adds a flatten-optional stage to this source.
  #[must_use]
  pub fn flatten_optional(self) -> Source<Out, Mat> {
    self.map_option(|value| value)
  }
}

impl<Out, Mat, Mat2> Source<Source<Out, Mat2>, Mat>
where
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
{
  /// Flattens nested sources.
  #[must_use]
  pub fn flatten(mut self) -> Source<Out, Mat> {
    let definition = flat_map_concat_definition::<Source<Out, Mat2>, Out, Mat2, _>(|source| source);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      self.graph.connect_or_panic(
        &Outlet::<Source<Out, Mat2>>::from_id(from),
        &Inlet::<Source<Out, Mat2>>::from_id(inlet_id),
        MatCombine::Left,
      );
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }
}

impl<Out, Mat> Source<Out, Mat>
where
  Out: Send + Sync + 'static,
{
  /// Maps upstream failures into different stream failures.
  #[must_use]
  pub fn map_error<F>(self, mapper: F) -> Source<Out, Mat>
  where
    F: FnMut(StreamError) -> StreamError + Send + Sync + 'static, {
    self.via(Flow::<Out, Out, StreamNotUsed>::new().map_error(mapper))
  }

  /// Resumes the stream when the upstream failure matches.
  #[must_use]
  pub fn on_error_continue(self) -> Source<Out, Mat> {
    self.via(Flow::<Out, Out, StreamNotUsed>::new().on_error_continue())
  }

  /// Resumes the stream and invokes `error_consumer` when the failure matches.
  #[must_use]
  pub fn on_error_continue_with<C>(self, error_consumer: C) -> Source<Out, Mat>
  where
    C: FnMut(&StreamError) + Send + Sync + 'static, {
    self.via(Flow::<Out, Out, StreamNotUsed>::new().on_error_continue_with(error_consumer))
  }

  /// Resumes the stream when the upstream failure matches `predicate`.
  #[must_use]
  pub fn on_error_continue_if<P>(self, predicate: P) -> Source<Out, Mat>
  where
    P: FnMut(&StreamError) -> bool + Send + Sync + 'static, {
    self.via(Flow::<Out, Out, StreamNotUsed>::new().on_error_continue_if(predicate))
  }

  /// Resumes the stream and invokes `error_consumer` when the failure matches `predicate`.
  #[must_use]
  pub fn on_error_continue_if_with<P, C>(self, predicate: P, error_consumer: C) -> Source<Out, Mat>
  where
    P: FnMut(&StreamError) -> bool + Send + Sync + 'static,
    C: FnMut(&StreamError) + Send + Sync + 'static, {
    self.via(Flow::<Out, Out, StreamNotUsed>::new().on_error_continue_if_with(predicate, error_consumer))
  }

  /// Alias of [`Source::on_error_continue`].
  #[must_use]
  pub fn on_error_resume(self) -> Source<Out, Mat> {
    self.on_error_continue()
  }

  /// Completes the stream when the upstream failure matches.
  #[must_use]
  pub fn on_error_complete(self) -> Source<Out, Mat> {
    self.via(Flow::<Out, Out, StreamNotUsed>::new().on_error_complete())
  }

  /// Completes the stream when the upstream failure matches `predicate`.
  #[must_use]
  pub fn on_error_complete_if<P>(self, predicate: P) -> Source<Out, Mat>
  where
    P: FnMut(&StreamError) -> bool + Send + Sync + 'static, {
    self.via(Flow::<Out, Out, StreamNotUsed>::new().on_error_complete_if(predicate))
  }

  /// Recovers an upstream failure with a single replacement element.
  #[must_use]
  pub fn recover<F>(self, recover: F) -> Source<Out, Mat>
  where
    F: FnMut(StreamError) -> Option<Out> + Send + Sync + 'static, {
    self.via(Flow::<Out, Out, StreamNotUsed>::new().recover(recover))
  }

  /// Recovers upstream failures by switching to alternate sources.
  #[must_use]
  pub fn recover_with_retries<F>(self, max_retries: isize, recover: F) -> Source<Out, Mat>
  where
    F: FnMut(StreamError) -> Option<Source<Out, StreamNotUsed>> + Send + Sync + 'static, {
    self.via(Flow::<Out, Out, StreamNotUsed>::new().recover_with_retries(max_retries, recover))
  }

  /// Alias of [`Source::recover_with_retries`] with infinite retries.
  #[must_use]
  pub fn recover_with<F>(self, recover: F) -> Source<Out, Mat>
  where
    F: FnMut(StreamError) -> Option<Source<Out, StreamNotUsed>> + Send + Sync + 'static, {
    self.recover_with_retries(-1, recover)
  }
}

impl<Out, Mat> Source<Out, Mat>
where
  Out: Clone + Ord + Send + Sync + 'static,
{
  /// Filters out elements that have already been seen, using `Ord` for tracking.
  #[must_use]
  pub fn distinct(self) -> Source<Out, Mat> {
    self
      .stateful_map(|| {
        let mut seen = BTreeSet::new();
        move |value: Out| {
          if seen.contains(&value) {
            return None;
          }
          seen.insert(value.clone());
          Some(value)
        }
      })
      .flatten_optional()
  }
}

impl<Out, Mat> Source<Out, Mat>
where
  Out: Send + Sync + 'static,
{
  /// Filters out elements whose key has already been seen.
  #[must_use]
  pub fn distinct_by<K, F>(self, key_fn: F) -> Source<Out, Mat>
  where
    K: Ord + Send + Sync + 'static,
    F: FnMut(&Out) -> K + Clone + Send + Sync + 'static, {
    self
      .stateful_map(move || {
        let mut key_fn = key_fn.clone();
        let mut seen = BTreeSet::<K>::new();
        move |value: Out| {
          let key = key_fn(&value);
          if seen.contains(&key) {
            return None;
          }
          seen.insert(key);
          Some(value)
        }
      })
      .flatten_optional()
  }
}

impl<Out> Source<Out, StreamCompletion<StreamDone>>
where
  Out: Send + Sync + 'static,
{
  /// Converts this source into a pre-materialized form.
  #[must_use]
  pub fn pre_materialize(self) -> (Self, StreamCompletion<StreamDone>) {
    let (graph, mat) = self.into_parts();
    let source = Source { graph, mat: mat.clone(), _pd: PhantomData };
    (source, mat)
  }
}

impl<Out, Mat> StreamStage for Source<Out, Mat> {
  type In = StreamNotUsed;
  type Out = Out;

  fn shape(&self) -> StreamShape<Self::In, Self::Out> {
    let outlet = self.graph.tail_outlet().map(Outlet::from_id).unwrap_or_default();
    StreamShape::new(Inlet::new(), outlet)
  }
}

struct SingleSourceLogic<Out> {
  value: Option<Out>,
}

struct EmptySourceLogic;

impl SourceLogic for EmptySourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(None)
  }

  fn should_drain_on_shutdown(&self) -> bool {
    true
  }
}

struct IteratorSourceLogic<I> {
  values: I,
}

struct ArraySourceLogic<Out, const N: usize> {
  values: IntoIter<Out, N>,
}

struct FailedSourceLogic {
  error: StreamError,
}

struct NeverSourceLogic;

struct RepeatSourceLogic<Out> {
  value: Out,
}

struct CycleSourceLogic<Out> {
  values: Vec<Out>,
  index:  usize,
}

struct IterateSourceLogic<Out, F> {
  current: Out,
  func:    F,
}

struct FutureSourceLogic<Out, Fut> {
  future: Option<Pin<Box<Fut>>>,
  done:   bool,
  _pd:    PhantomData<fn() -> Out>,
}

struct UnfoldAsyncSourceLogic<State, Out, F, Fut> {
  state:   Option<State>,
  func:    F,
  pending: Option<Pin<Box<Fut>>>,
  done:    bool,
  _pd:     PhantomData<fn() -> Out>,
}

struct QueueSourceLogic<Out> {
  queue: BoundedSourceQueue<Out>,
}

struct QueueWithOverflowSourceLogic<Out> {
  queue: SourceQueueWithComplete<Out>,
}

struct UnboundedQueueSourceLogic<Out> {
  queue: SourceQueue<Out>,
}

impl<Out, I> SourceLogic for IteratorSourceLogic<I>
where
  Out: Send + 'static,
  I: Iterator<Item = Out> + Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(self.values.next().map(|value| Box::new(value) as DynValue))
  }

  fn should_drain_on_shutdown(&self) -> bool {
    true
  }
}

impl<Out, const N: usize> SourceLogic for ArraySourceLogic<Out, N>
where
  Out: Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(self.values.next().map(|value| Box::new(value) as DynValue))
  }

  fn should_drain_on_shutdown(&self) -> bool {
    true
  }
}

impl SourceLogic for FailedSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Err(self.error.clone())
  }
}

impl SourceLogic for NeverSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Err(StreamError::WouldBlock)
  }
}

impl<Out> SourceLogic for RepeatSourceLogic<Out>
where
  Out: Clone + Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(Some(Box::new(self.value.clone()) as DynValue))
  }
}

impl<Out> SourceLogic for CycleSourceLogic<Out>
where
  Out: Clone + Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    if self.values.is_empty() {
      return Ok(None);
    }
    let value = self.values[self.index].clone();
    self.index = (self.index + 1) % self.values.len();
    Ok(Some(Box::new(value) as DynValue))
  }
}

impl<Out, F> SourceLogic for IterateSourceLogic<Out, F>
where
  Out: Clone + Send + 'static,
  F: FnMut(Out) -> Out + Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    let next = (self.func)(self.current.clone());
    let value = core::mem::replace(&mut self.current, next);
    Ok(Some(Box::new(value) as DynValue))
  }
}

impl<Out> SourceLogic for SingleSourceLogic<Out>
where
  Out: Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(self.value.take().map(|value| Box::new(value) as DynValue))
  }

  fn should_drain_on_shutdown(&self) -> bool {
    true
  }
}

impl<Out, Fut> SourceLogic for FutureSourceLogic<Out, Fut>
where
  Out: Send + 'static,
  Fut: Future<Output = Out> + Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    if self.done {
      return Ok(None);
    }
    let Some(future) = self.future.as_mut() else {
      self.done = true;
      return Ok(None);
    };
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    match future.as_mut().poll(&mut cx) {
      | Poll::Ready(value) => {
        self.done = true;
        self.future = None;
        Ok(Some(Box::new(value) as DynValue))
      },
      | Poll::Pending => Err(StreamError::WouldBlock),
    }
  }
}

impl<State, Out, F, Fut> SourceLogic for UnfoldAsyncSourceLogic<State, Out, F, Fut>
where
  State: Send + 'static,
  Out: Send + 'static,
  F: FnMut(State) -> Fut + Send + Sync + 'static,
  Fut: Future<Output = Option<(State, Out)>> + Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    if self.done {
      return Ok(None);
    }
    if self.pending.is_none() {
      let Some(state) = self.state.take() else {
        self.done = true;
        return Ok(None);
      };
      self.pending = Some(Box::pin((self.func)(state)));
    }
    let Some(pending) = self.pending.as_mut() else {
      self.done = true;
      return Ok(None);
    };
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    match pending.as_mut().poll(&mut cx) {
      | Poll::Ready(Some((next_state, output))) => {
        self.state = Some(next_state);
        self.pending = None;
        Ok(Some(Box::new(output) as DynValue))
      },
      | Poll::Ready(None) => {
        self.done = true;
        self.pending = None;
        Ok(None)
      },
      | Poll::Pending => Err(StreamError::WouldBlock),
    }
  }
}

impl<Out> SourceLogic for QueueSourceLogic<Out>
where
  Out: Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    match self.queue.poll_or_drain()? {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Ok(None),
    }
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    self.queue.close_for_cancel();
    Ok(())
  }
}

impl<Out> SourceLogic for QueueWithOverflowSourceLogic<Out>
where
  Out: Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    match self.queue.poll_or_drain()? {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Ok(None),
    }
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    self.queue.close_for_cancel();
    Ok(())
  }
}

impl<Out> SourceLogic for UnboundedQueueSourceLogic<Out>
where
  Out: Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    match self.queue.poll_or_drain()? {
      | Some(value) => Ok(Some(Box::new(value) as DynValue)),
      | None => Ok(None),
    }
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    self.queue.close_for_cancel();
    Ok(())
  }
}

struct LazySourceLogic<Out, F> {
  factory: Option<F>,
  buffer:  VecDeque<DynValue>,
  // factory 消費後に nested source の評価が失敗した場合のエラー状態
  error:   Option<StreamError>,
  _pd:     PhantomData<fn() -> Out>,
}

impl<Out, F> SourceLogic for LazySourceLogic<Out, F>
where
  Out: Send + Sync + 'static,
  F: FnOnce() -> Source<Out, StreamNotUsed> + Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    if let Some(error) = &self.error {
      return Err(error.clone());
    }
    if let Some(factory) = self.factory.take() {
      let source = factory();
      match drain_source_for_lazy_source(source) {
        | Ok(values) => {
          self.buffer = values.into_iter().map(|v| Box::new(v) as DynValue).collect();
        },
        | Err(e) => {
          self.error = Some(e.clone());
          return Err(e);
        },
      }
    }
    Ok(self.buffer.pop_front())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    if let Some(error) = &self.error {
      return Err(error.clone());
    }
    Ok(())
  }
}

fn drain_source_for_lazy_source<Out>(source: Source<Out, StreamNotUsed>) -> Result<Vec<Out>, StreamError>
where
  Out: Send + Sync + 'static, {
  let mut graph = source.graph;
  let Some(tail_outlet_id) = graph.tail_outlet() else {
    return Err(StreamError::InvalidConnection);
  };
  let tail_outlet = Outlet::<Out>::from_id(tail_outlet_id);
  let sink = Sink::collect();
  let (sink_graph, completion) = sink.into_parts();
  let sink_inlet_id = sink_graph.head_inlet().ok_or(StreamError::InvalidConnection)?;
  graph.append(sink_graph);
  let sink_inlet = Inlet::<Out>::from_id(sink_inlet_id);

  if let Some(expected_fan_out) = graph.expected_fan_out_for_outlet(tail_outlet_id) {
    for _ in 1..expected_fan_out {
      // lazy_source でも multi-outlet source の fan-out 契約どおりに各複製を収集する。
      let branch = map_definition::<Out, Out, _>(|value| value);
      let branch_inlet = Inlet::<Out>::from_id(branch.inlet);
      let branch_outlet = Outlet::<Out>::from_id(branch.outlet);
      graph.push_stage(StageDefinition::Flow(branch));
      graph.connect_or_panic(&tail_outlet, &branch_inlet, MatCombine::Left);
      graph.connect_or_panic(&branch_outlet, &sink_inlet, MatCombine::Right);
    }
  }

  let plan = graph.into_plan()?;
  let island_plan = IslandSplitter::split(plan);
  if island_plan.islands().len() <= 1 {
    let mut stream = Stream::new(island_plan.into_single_plan(), StreamBufferConfig::default());
    stream.start()?;
    let mut idle_budget = 1024_usize;
    while !stream.state().is_terminal() {
      match stream.drive() {
        | DriveOutcome::Progressed => idle_budget = 1024,
        | DriveOutcome::Idle => {
          if idle_budget == 0 {
            return Err(StreamError::WouldBlock);
          }
          idle_budget = idle_budget.saturating_sub(1);
        },
      }
    }
  } else {
    let (mut islands, crossings) = island_plan.into_parts();
    let mut boundaries = Vec::with_capacity(crossings.len());
    for crossing in crossings {
      let upstream_idx = crossing.from_island().as_usize();
      let downstream_idx = crossing.to_island().as_usize();
      let boundary_capacity = islands[downstream_idx]
        .input_buffer_capacity_for_inlet(crossing.to_port())
        .unwrap_or(DEFAULT_BOUNDARY_CAPACITY);
      let boundary = IslandBoundaryShared::new(boundary_capacity);
      boundaries.push((boundary.clone(), downstream_idx));
      islands[upstream_idx].add_boundary_sink(boundary.clone(), crossing.from_port(), crossing.element_type());
      islands[downstream_idx].add_boundary_source(boundary, crossing.to_port(), crossing.element_type());
    }
    let mut streams = Vec::with_capacity(islands.len());
    for island in islands {
      let mut stream = Stream::new(island.into_stream_plan(), StreamBufferConfig::default());
      stream.start()?;
      streams.push(stream);
    }
    let mut idle_budget = 4096_usize;
    while streams.iter().any(|stream| !stream.state().is_terminal()) {
      let mut progressed = false;
      for stream in &mut streams {
        if !stream.state().is_terminal() && matches!(stream.drive(), DriveOutcome::Progressed) {
          progressed = true;
        }
      }
      if progressed {
        idle_budget = 4096;
      } else if idle_budget == 0
        || boundaries
          .iter()
          .any(|(boundary, downstream_idx)| boundary.is_full() && streams[*downstream_idx].state().is_terminal())
      {
        return Err(StreamError::WouldBlock);
      } else {
        idle_budget = idle_budget.saturating_sub(1);
      }
    }
  }
  completion.try_take().unwrap_or(Err(StreamError::Failed))
}

impl<Out> GraphStageLogic<StreamNotUsed, Out, StreamNotUsed> for SingleSourceLogic<Out>
where
  Out: Send + Sync + 'static,
{
  fn on_pull(&mut self, ctx: &mut dyn StageContext<StreamNotUsed, Out>) {
    match self.value.take() {
      | Some(value) => ctx.push(value),
      | None => ctx.complete(),
    }
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

impl<Out> GraphStage<StreamNotUsed, Out, StreamNotUsed> for SingleSourceLogic<Out>
where
  Out: Send + Sync + 'static + Clone,
{
  fn shape(&self) -> StreamShape<StreamNotUsed, Out> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<StreamNotUsed, Out, StreamNotUsed> + Send> {
    Box::new(SingleSourceLogic { value: self.value.clone() })
  }
}

fn combine_mat<Left, Right, C>(left: Left, right: Right) -> C::Out
where
  C: MatCombineRule<Left, Right>, {
  C::combine(left, right)
}

const fn noop_waker() -> Waker {
  unsafe { Waker::from_raw(noop_raw_waker()) }
}

const fn noop_raw_waker() -> RawWaker {
  RawWaker::new(core::ptr::null(), &NOOP_WAKER_VTABLE)
}

const fn noop_clone(_: *const ()) -> RawWaker {
  noop_raw_waker()
}

const fn noop_wake(_: *const ()) {}

const fn noop_wake_by_ref(_: *const ()) {}

const fn noop_drop(_: *const ()) {}

const NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop_wake, noop_wake_by_ref, noop_drop);
