use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use core::{
  any::TypeId,
  future::Future,
  marker::PhantomData,
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use fraktor_utils_rs::core::collections::queue::OverflowPolicy;

use super::{
  DynValue, MatCombine, MatCombineRule, Materialized, Materializer, RestartBackoff, RestartSettings, RunnableGraph,
  SourceDefinition, SourceLogic, SourceSubFlow, StageDefinition, StageKind, StreamCompletion, StreamDone,
  StreamDslError, StreamError, StreamGraph, StreamNotUsed, StreamStage, SupervisionStrategy,
  flow::{
    async_boundary_definition, balance_definition, batch_definition, broadcast_definition, buffer_definition,
    concat_definition, concat_substreams_definition, delay_definition, drop_definition, drop_while_definition,
    filter_definition, flat_map_concat_definition, flat_map_merge_definition, group_by_definition, grouped_definition,
    initial_delay_definition, interleave_definition, intersperse_definition, map_async_definition,
    map_concat_definition, map_definition, map_option_definition, merge_definition, merge_substreams_definition,
    merge_substreams_with_parallelism_definition, partition_definition, prepend_definition, recover_definition,
    recover_with_retries_definition, scan_definition, sliding_definition, split_after_definition,
    split_when_definition, stateful_map_concat_definition, stateful_map_definition, take_definition,
    take_until_definition, take_while_definition, take_within_definition, throttle_definition, unzip_definition,
    unzip_with_definition, zip_all_definition, zip_definition, zip_with_index_definition,
  },
  graph::{GraphStage, GraphStageLogic},
  shape::{Inlet, Outlet, StreamShape},
  sink::Sink,
  stage_context::StageContext,
  validate_positive_argument,
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
  Out: Send + Sync + 'static,
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
    Self::from_iterator(values)
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
      mat_combine: MatCombine::KeepRight,
      supervision: SupervisionStrategy::Stop,
      restart:     None,
      logic:       Box::new(logic),
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

  /// Converts this source into a context-carrying source by attaching unit context.
  #[must_use]
  pub fn as_source_with_context(self) -> Source<((), Out), StreamNotUsed> {
    self.map(|value| ((), value))
  }

  /// Creates a sink endpoint that can be paired with a source subscriber bridge.
  #[must_use]
  pub fn as_subscriber() -> Sink<Out, StreamCompletion<StreamDone>> {
    Sink::ignore()
  }

  /// Creates a source from actor-ref style push values.
  #[must_use]
  pub fn actor_ref<I>(values: I) -> Self
  where
    I: IntoIterator<Item = Out>,
    I::IntoIter: Send + 'static, {
    Self::from_iterator(values)
  }

  /// Creates a source from actor-ref with backpressure style push values.
  #[must_use]
  pub fn actor_ref_with_backpressure<I>(values: I) -> Self
  where
    I: IntoIterator<Item = Out>,
    I::IntoIter: Send + 'static, {
    Self::actor_ref(values)
  }

  /// Creates a sink endpoint for actor interop entry points.
  #[must_use]
  pub fn sink() -> Sink<Out, StreamCompletion<StreamDone>> {
    Self::as_subscriber()
  }

  /// Adds an actor-watch compatibility stage.
  #[must_use]
  pub const fn watch(self) -> Self {
    self
  }

  /// Combines multiple sources by selecting the first source when available.
  #[must_use]
  pub fn combine<I>(sources: I) -> Self
  where
    I: IntoIterator<Item = Self>, {
    sources.into_iter().next().unwrap_or_else(Self::empty)
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
  #[must_use]
  pub fn lazy_single<F>(factory: F) -> Self
  where
    F: FnOnce() -> Out, {
    Self::single(factory())
  }

  /// Lazily creates a source from a source factory.
  ///
  /// The factory is not called until the first element is demanded.
  /// All elements from the created source are collected and buffered on first pull.
  #[must_use]
  pub fn lazy_source<F>(factory: F) -> Self
  where
    F: FnOnce() -> Self + Send + 'static, {
    Self::from_logic(
      StageKind::Custom,
      LazySourceLogic::<Out, F> { factory: Some(factory), buffer: VecDeque::new(), _pd: PhantomData },
    )
  }

  /// Creates an optional source.
  #[must_use]
  pub fn maybe(value: Option<Out>) -> Self {
    Self::from_option(value)
  }

  /// Creates a source backed by queue-compatible values.
  #[must_use]
  pub fn queue<I>(values: I) -> Self
  where
    I: IntoIterator<Item = Out>,
    I::IntoIter: Send + 'static, {
    Self::from_iterator(values)
  }

  /// Creates a ticking source by repeating and delaying values.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `interval_ticks` is zero.
  pub fn tick(initial_delay_ticks: usize, interval_ticks: usize, value: Out) -> Result<Self, StreamDslError>
  where
    Out: Clone, {
    let _ = validate_positive_argument("interval_ticks", interval_ticks)?;
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
  #[must_use]
  pub fn zip_n(self, n: usize) -> Source<Vec<Out>, StreamNotUsed> {
    self.zip(n)
  }

  /// Alias of [`Source::zip_n`] followed by mapping.
  #[must_use]
  pub fn zip_with_n<T, F>(self, n: usize, func: F) -> Source<T, StreamNotUsed>
  where
    T: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> T + Send + Sync + 'static, {
    self.zip_n(n).map(func)
  }

  pub(in crate::core) fn from_logic<L>(kind: StageKind, logic: L) -> Self
  where
    L: SourceLogic + 'static, {
    let mut graph = StreamGraph::new();
    let outlet: Outlet<Out> = Outlet::new();
    let definition = SourceDefinition {
      kind,
      outlet: outlet.id(),
      output_type: TypeId::of::<Out>(),
      mat_combine: MatCombine::KeepRight,
      supervision: SupervisionStrategy::Stop,
      restart: None,
      logic: Box::new(logic),
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

impl Source<u8, StreamNotUsed> {
  /// Creates a source from a path-compatible value.
  #[must_use]
  pub fn from_path(path: &str) -> Self {
    Self::from_iterator(path.as_bytes().to_vec())
  }
}

impl<Out, Mat> Source<Out, Mat>
where
  Out: Send + Sync + 'static,
{
  /// Composes this source with a flow.
  #[must_use]
  pub fn via<T, Mat2>(self, flow: super::flow::Flow<Out, T, Mat2>) -> Source<T, Mat>
  where
    T: Send + Sync + 'static, {
    self.via_mat(flow, super::keep_left::KeepLeft)
  }

  /// Composes this source with a flow using a custom materialized rule.
  #[must_use]
  pub fn via_mat<T, Mat2, C>(self, flow: super::flow::Flow<Out, T, Mat2>, _combine: C) -> Source<T, C::Out>
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

  /// Connects this source to a sink.
  #[must_use]
  pub fn to<Mat2>(self, sink: Sink<Out, Mat2>) -> RunnableGraph<Mat> {
    self.to_mat(sink, super::keep_left::KeepLeft)
  }

  /// Connects this source to a sink using a custom materialized rule.
  ///
  /// # Panics
  ///
  /// Panics when the stream graph cannot be converted into a runnable plan.
  #[must_use]
  pub fn to_mat<Mat2, C>(self, sink: Sink<Out, Mat2>, _combine: C) -> RunnableGraph<C::Out>
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
  ) -> Result<Materialized<Mat2, M::Toolbox>, StreamError>
  where
    M: Materializer, {
    self.to_mat(sink, super::keep_right::KeepRight).run(materializer)
  }

  /// Adds a map stage to this source.
  #[must_use]
  pub fn map<T, F>(mut self, func: F) -> Source<T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> T + Send + Sync + 'static, {
    let definition = map_definition::<Out, T, F>(func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
    overflow_policy: OverflowPolicy,
  ) -> Result<Source<Out, Mat>, StreamDslError> {
    let capacity = validate_positive_argument("capacity", capacity)?;
    let definition = buffer_definition::<Out>(capacity, overflow_policy);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Decouples upstream and downstream demand signaling via an async boundary.
  #[must_use]
  pub fn detach(self) -> Source<Out, Mat> {
    self.async_boundary()
  }

  /// Adds an explicit async boundary stage.
  #[must_use]
  pub fn async_boundary(mut self) -> Source<Out, Mat> {
    let definition = async_boundary_definition::<Out>();
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a throttle stage that limits the number of buffered in-flight elements.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `capacity` is zero.
  pub fn throttle(mut self, capacity: usize) -> Result<Source<Out, Mat>, StreamDslError> {
    let capacity = validate_positive_argument("capacity", capacity)?;
    let definition = throttle_definition::<Out>(capacity);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Source { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Enables restart semantics with backoff for this source.
  #[must_use]
  pub fn restart_source_with_backoff(mut self, min_backoff_ticks: u32, max_restarts: usize) -> Source<Out, Mat> {
    self.graph.set_source_restart(Some(RestartBackoff::new(min_backoff_ticks, max_restarts)));
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
  pub fn restart_source_with_settings(mut self, settings: RestartSettings) -> Source<Out, Mat> {
    self.graph.set_source_restart(Some(RestartBackoff::from_settings(settings)));
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

  /// Adds a group-by stage and returns substream surface for merge operations.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `max_substreams` is zero.
  pub fn group_by<Key, F>(
    mut self,
    max_substreams: usize,
    key_fn: F,
  ) -> Result<SourceSubFlow<Out, Mat>, StreamDslError>
  where
    Key: Clone + PartialEq + Send + Sync + 'static,
    F: FnMut(&Out) -> Key + Send + Sync + 'static, {
    let max_substreams = validate_positive_argument("max_substreams", max_substreams)?;
    let definition = group_by_definition::<Out, Key, F>(max_substreams, key_fn);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    let grouped = Source::<(Key, Out), Mat> { graph: self.graph, mat: self.mat, _pd: PhantomData };
    Ok(SourceSubFlow::from_source(grouped.map(|(_, value)| vec![value])))
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a broadcast stage that duplicates each element `fan_out` times.
  ///
  /// # Panics
  ///
  /// Panics when `fan_out` is zero.
  #[must_use]
  pub fn broadcast(mut self, fan_out: usize) -> Source<Out, Mat>
  where
    Out: Clone, {
    assert!(fan_out > 0, "fan_out must be greater than zero");
    let definition = broadcast_definition::<Out>(fan_out);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a balance stage that distributes elements across `fan_out` outputs.
  ///
  /// # Panics
  ///
  /// Panics when `fan_out` is zero.
  #[must_use]
  pub fn balance(mut self, fan_out: usize) -> Source<Out, Mat> {
    assert!(fan_out > 0, "fan_out must be greater than zero");
    let definition = balance_definition::<Out>(fan_out);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a merge stage that merges `fan_in` upstream paths.
  ///
  /// # Panics
  ///
  /// Panics when `fan_in` is zero.
  #[must_use]
  pub fn merge(mut self, fan_in: usize) -> Source<Out, Mat> {
    assert!(fan_in > 0, "fan_in must be greater than zero");
    let definition = merge_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds an interleave stage that consumes `fan_in` inputs in round-robin order.
  ///
  /// # Panics
  ///
  /// Panics when `fan_in` is zero.
  #[must_use]
  pub fn interleave(mut self, fan_in: usize) -> Source<Out, Mat> {
    assert!(fan_in > 0, "fan_in must be greater than zero");
    let definition = interleave_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a prepend stage that prioritizes lower-index input lanes.
  ///
  /// # Panics
  ///
  /// Panics when `fan_in` is zero.
  #[must_use]
  pub fn prepend(mut self, fan_in: usize) -> Source<Out, Mat> {
    assert!(fan_in > 0, "fan_in must be greater than zero");
    let definition = prepend_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a zip stage that emits one vector after receiving one element from each input.
  ///
  /// # Panics
  ///
  /// Panics when `fan_in` is zero.
  #[must_use]
  pub fn zip(mut self, fan_in: usize) -> Source<Vec<Out>, Mat> {
    assert!(fan_in > 0, "fan_in must be greater than zero");
    let definition = zip_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a zip-all stage that fills missing lanes with `fill_value` after completion.
  ///
  /// # Panics
  ///
  /// Panics when `fan_in` is zero.
  #[must_use]
  pub fn zip_all(mut self, fan_in: usize, fill_value: Out) -> Source<Vec<Out>, Mat>
  where
    Out: Clone, {
    assert!(fan_in > 0, "fan_in must be greater than zero");
    let definition = zip_all_definition::<Out>(fan_in, fill_value);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a zip-with-index stage that pairs each element with an incrementing index.
  #[must_use]
  pub fn zip_with_index(mut self) -> Source<(Out, u64), Mat> {
    let definition = zip_with_index_definition::<Out>();
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a concat stage that emits all elements from each input in port order.
  ///
  /// # Panics
  ///
  /// Panics when `fan_in` is zero.
  #[must_use]
  pub fn concat(mut self, fan_in: usize) -> Source<Out, Mat> {
    assert!(fan_in > 0, "fan_in must be greater than zero");
    let definition = concat_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Runs this source to completion and collects emitted elements.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when graph construction or execution fails.
  pub fn collect_values(self) -> Result<Vec<Out>, StreamError> {
    let mut graph = self.graph;
    let Some(tail_outlet_id) = graph.tail_outlet() else {
      return Err(StreamError::InvalidConnection);
    };
    let tail_outlet = Outlet::<Out>::from_id(tail_outlet_id);
    let sink = Sink::fold(Vec::new(), |mut acc: Vec<Out>, value| {
      acc.push(value);
      acc
    });
    let (sink_graph, completion) = sink.into_parts();
    let Some(sink_inlet_id) = sink_graph.head_inlet() else {
      return Err(StreamError::InvalidConnection);
    };
    graph.append(sink_graph);
    let sink_inlet = Inlet::<Out>::from_id(sink_inlet_id);

    if let Some(expected_fan_out) = graph.expected_fan_out_for_outlet(tail_outlet_id) {
      for _ in 1..expected_fan_out {
        let branch = map_definition::<Out, Out, _>(|value| value);
        let branch_inlet = Inlet::<Out>::from_id(branch.inlet);
        let branch_outlet = Outlet::<Out>::from_id(branch.outlet);
        graph.push_stage(StageDefinition::Flow(branch));
        graph.connect(&tail_outlet, &branch_inlet, MatCombine::KeepLeft)?;
        graph.connect(&branch_outlet, &sink_inlet, MatCombine::KeepRight)?;
      }
    }

    let plan = graph.into_plan()?;
    let mut stream = super::lifecycle::Stream::new(plan, super::StreamBufferConfig::default());
    stream.start()?;
    let mut idle_budget = 1024_usize;
    while !stream.state().is_terminal() {
      match stream.drive() {
        | super::DriveOutcome::Progressed => idle_budget = 1024,
        | super::DriveOutcome::Idle => {
          if idle_budget == 0 {
            return Err(StreamError::WouldBlock);
          }
          idle_budget = idle_budget.saturating_sub(1);
        },
      }
    }
    match completion.try_take() {
      | Some(result) => result,
      | None => Err(StreamError::Failed),
    }
  }

  /// Converts this source into an input-stream compatible collection.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when source execution fails.
  pub fn as_input_stream(self) -> Result<Vec<Out>, StreamError> {
    self.collect_values()
  }

  /// Converts this source into a Java-stream compatible collection.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when source execution fails.
  pub fn as_java_stream(self) -> Result<Vec<Out>, StreamError> {
    self.collect_values()
  }

  /// Converts this source into an output-stream compatible collection.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when source execution fails.
  pub fn as_output_stream(self) -> Result<Vec<Out>, StreamError> {
    self.collect_values()
  }

  pub(crate) fn into_parts(self) -> (StreamGraph, Mat) {
    (self.graph, self.mat)
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
      let _ = self.graph.connect(
        &Outlet::<(Out, Out)>::from_id(from),
        &Inlet::<(Out, Out)>::from_id(inlet_id),
        MatCombine::KeepLeft,
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
      let _ = self.graph.connect(
        &Outlet::<Vec<Out>>::from_id(from),
        &Inlet::<Vec<Out>>::from_id(inlet_id),
        MatCombine::KeepLeft,
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
      let _ = self.graph.connect(
        &Outlet::<Vec<Out>>::from_id(from),
        &Inlet::<Vec<Out>>::from_id(inlet_id),
        MatCombine::KeepLeft,
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
      let _ = self.graph.connect(
        &Outlet::<Vec<Out>>::from_id(from),
        &Inlet::<Vec<Out>>::from_id(inlet_id),
        MatCombine::KeepLeft,
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

impl<Out, Mat> Source<Result<Out, StreamError>, Mat>
where
  Out: Clone + Send + Sync + 'static,
{
  /// Maps error payloads while keeping successful elements unchanged.
  #[must_use]
  pub fn map_error<F>(self, mut mapper: F) -> Source<Result<Out, StreamError>, Mat>
  where
    F: FnMut(StreamError) -> StreamError + Send + Sync + 'static, {
    self.map(move |value| value.map_err(&mut mapper))
  }

  /// Drops failing payloads and keeps successful elements.
  #[must_use]
  pub fn on_error_continue(self) -> Source<Out, Mat> {
    self.map_option(Result::ok)
  }

  /// Alias of [`Source::on_error_continue`].
  #[must_use]
  pub fn on_error_resume(self) -> Source<Out, Mat> {
    self.on_error_continue()
  }

  /// Emits successful payloads until first error payload is observed.
  #[must_use]
  pub fn on_error_complete(self) -> Source<Out, Mat> {
    self
      .stateful_map(|| {
        let mut seen_error = false;
        move |value| {
          if seen_error {
            return None;
          }
          match value {
            | Ok(value) => Some(value),
            | Err(_) => {
              seen_error = true;
              None
            },
          }
        }
      })
      .flatten_optional()
  }

  /// Recovers error payloads with the provided fallback element.
  #[must_use]
  pub fn recover(mut self, fallback: Out) -> Source<Out, Mat> {
    let definition = recover_definition::<Out>(fallback);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(
        &Outlet::<Result<Out, StreamError>>::from_id(from),
        &Inlet::<Result<Out, StreamError>>::from_id(inlet_id),
        MatCombine::KeepLeft,
      );
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Recovers error payloads while retry budget remains, then fails the stream.
  #[must_use]
  pub fn recover_with_retries(mut self, max_retries: usize, fallback: Out) -> Source<Out, Mat> {
    let definition = recover_with_retries_definition::<Out>(max_retries, fallback);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(
        &Outlet::<Result<Out, StreamError>>::from_id(from),
        &Inlet::<Result<Out, StreamError>>::from_id(inlet_id),
        MatCombine::KeepLeft,
      );
    }
    Source { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Alias of [`Source::recover`].
  #[must_use]
  pub fn recover_with(self, fallback: Out) -> Source<Out, Mat> {
    self.recover(fallback)
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
}

struct IteratorSourceLogic<I> {
  values: I,
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

impl<Out, I> SourceLogic for IteratorSourceLogic<I>
where
  Out: Send + Sync + 'static,
  I: Iterator<Item = Out> + Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(self.values.next().map(|value| Box::new(value) as DynValue))
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
  Out: Clone + Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(Some(Box::new(self.value.clone()) as DynValue))
  }
}

impl<Out> SourceLogic for CycleSourceLogic<Out>
where
  Out: Clone + Send + Sync + 'static,
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
  Out: Clone + Send + Sync + 'static,
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
  Out: Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(self.value.take().map(|value| Box::new(value) as DynValue))
  }
}

impl<Out, Fut> SourceLogic for FutureSourceLogic<Out, Fut>
where
  Out: Send + Sync + 'static,
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
  Out: Send + Sync + 'static,
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

struct LazySourceLogic<Out, F> {
  factory: Option<F>,
  buffer:  VecDeque<DynValue>,
  _pd:     PhantomData<fn() -> Out>,
}

impl<Out, F> SourceLogic for LazySourceLogic<Out, F>
where
  Out: Send + Sync + 'static,
  F: FnOnce() -> Source<Out, StreamNotUsed> + Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    if let Some(factory) = self.factory.take() {
      let source = factory();
      let values = source.collect_values()?;
      self.buffer = values.into_iter().map(|v| Box::new(v) as DynValue).collect();
    }
    Ok(self.buffer.pop_front())
  }
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

  fn create_logic(&self) -> Box<dyn GraphStageLogic<StreamNotUsed, Out, StreamNotUsed>> {
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
