use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use core::{
  any::TypeId,
  future::Future,
  marker::PhantomData,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use super::{
  FlowDefinition, FlowGroupBySubFlow, FlowLogic, FlowMonitor, FlowSubFlow, MatCombine, MatCombineRule,
  OverflowStrategy, RestartBackoff, RestartSettings, Source, StageDefinition, StageKind, StreamDslError, StreamError,
  StreamGraph, StreamNotUsed, StreamStage, SupervisionStrategy, TailSource,
  shape::{Inlet, Outlet, StreamShape},
  sink::Sink,
  validate_positive_argument,
};
use crate::core::{
  Attributes, DynValue, KeepRight, SourceLogic, StreamBufferConfig, StreamCompletion, SubstreamCancelStrategy,
  lifecycle::{DriveOutcome, KillSwitchStateHandle, Stream},
};

#[cfg(test)]
mod tests;

mod logic;
use logic::*;

/// Flow stage definition.
pub struct Flow<In, Out, Mat> {
  graph: StreamGraph,
  mat:   Mat,
  _pd:   PhantomData<fn(In) -> Out>,
}

impl<In, Out, Mat> Flow<In, Out, Mat> {
  /// Creates a flow from a pre-built stream graph and materialized value.
  #[must_use]
  pub(crate) fn from_graph(graph: StreamGraph, mat: Mat) -> Self {
    Self { graph, mat, _pd: PhantomData }
  }

  pub(crate) fn into_parts(self) -> (StreamGraph, Mat) {
    (self.graph, self.mat)
  }
}

impl<T> Flow<T, T, StreamNotUsed> {
  /// Creates an identity flow.
  #[must_use]
  pub fn new() -> Self {
    Self { graph: StreamGraph::new(), mat: StreamNotUsed::new(), _pd: PhantomData }
  }
}

impl<T> Flow<T, T, StreamNotUsed>
where
  T: Send + Sync + 'static,
{
  pub(in crate::core) fn from_kill_switch_state(kill_switch_state: KillSwitchStateHandle) -> Self {
    let mut graph = StreamGraph::new();
    graph.push_stage(StageDefinition::Flow(kill_switch_definition::<T>(kill_switch_state)));
    Self { graph, mat: StreamNotUsed::new(), _pd: PhantomData }
  }
}

impl<T> Default for Flow<T, T, StreamNotUsed> {
  fn default() -> Self {
    Self::new()
  }
}

impl<In, Out> Flow<In, Out, StreamNotUsed>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Creates a flow from a mapping function.
  #[must_use]
  pub fn from_function<F>(f: F) -> Self
  where
    F: Fn(In) -> Out + Send + Sync + 'static, {
    Flow::new().map(f)
  }

  /// Creates a flow from a materializer-provided factory.
  ///
  /// The factory is called eagerly to produce the flow.
  #[must_use]
  pub fn from_materializer<F>(factory: F) -> Self
  where
    F: FnOnce() -> Self, {
    factory()
  }
}

impl<In, Out, Mat> Flow<In, Out, Mat>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Composes this flow with the provided flow.
  #[must_use]
  pub fn via<T, Mat2>(self, flow: Flow<Out, T, Mat2>) -> Flow<In, T, Mat>
  where
    T: Send + Sync + 'static, {
    self.via_mat(flow, super::keep_left::KeepLeft)
  }

  /// Composes this flow with a custom materialized value rule.
  #[must_use]
  pub fn via_mat<T, Mat2, C>(self, flow: Flow<Out, T, Mat2>, _combine: C) -> Flow<In, T, C::Out>
  where
    T: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (mut graph, left_mat) = self.into_parts();
    let (flow_graph, right_mat) = flow.into_parts();
    graph.append(flow_graph);
    let mat = combine_mat::<Mat, Mat2, C>(left_mat, right_mat);
    Flow { graph, mat, _pd: PhantomData }
  }

  /// Maps this flow materialized value.
  #[must_use]
  pub fn map_materialized_value<Mat2, F>(self, func: F) -> Flow<In, Out, Mat2>
  where
    F: FnOnce(Mat) -> Mat2, {
    let (graph, mat) = self.into_parts();
    Flow::from_graph(graph, func(mat))
  }

  /// Connects this flow to a sink.
  #[must_use]
  pub fn to<Mat2>(self, sink: Sink<Out, Mat2>) -> Sink<In, Mat> {
    self.to_mat(sink, super::keep_left::KeepLeft)
  }

  /// Connects this flow to a sink with a custom materialized value rule.
  #[must_use]
  pub fn to_mat<Mat2, C>(self, sink: Sink<Out, Mat2>, _combine: C) -> Sink<In, C::Out>
  where
    C: MatCombineRule<Mat, Mat2>, {
    let (mut graph, left_mat) = self.into_parts();
    let (sink_graph, right_mat) = sink.into_parts();
    graph.append(sink_graph);
    let mat = combine_mat::<Mat, Mat2, C>(left_mat, right_mat);
    Sink::from_graph(graph, mat)
  }

  /// Adds a map stage to this flow.
  #[must_use]
  /// Note: `T` requires only `Send` (not `Sync`) to support `flatten()` and
  /// `flat_map_concat()` where `T = Source<_, _>` which is `!Sync`.
  /// Downstream operators that require `Sync` enforce it at their own boundary.
  pub fn map<T, F>(mut self, func: F) -> Flow<In, T, Mat>
  where
    T: Send + 'static,
    F: FnMut(Out) -> T + Send + Sync + 'static, {
    let definition = map_definition::<Out, T, F>(func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds an async map stage to this flow.
  ///
  /// This is a compatibility entry point for Pekko's `map_async`.
  /// `parallelism` is validated as a positive integer.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn map_async<T, F, Fut>(mut self, parallelism: usize, func: F) -> Result<Flow<In, T, Mat>, StreamDslError>
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
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a stateful-map stage to this flow.
  #[must_use]
  pub fn stateful_map<T, Factory, Mapper>(mut self, factory: Factory) -> Flow<In, T, Mat>
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a stateful-map-concat stage to this flow.
  #[must_use]
  pub fn stateful_map_concat<T, Factory, Mapper, I>(mut self, factory: Factory) -> Flow<In, T, Mat>
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a map-concat stage to this flow.
  #[must_use]
  pub fn map_concat<T, F, I>(mut self, func: F) -> Flow<In, T, Mat>
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a map-option stage to this flow.
  #[must_use]
  pub fn map_option<T, F>(mut self, func: F) -> Flow<In, T, Mat>
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a filter stage to this flow.
  #[must_use]
  pub fn filter<F>(mut self, predicate: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = filter_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a filter-not stage to this flow.
  #[must_use]
  pub fn filter_not<F>(self, mut predicate: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    self.filter(move |value| !predicate(value))
  }

  /// Adds a drop stage that skips the first `count` elements.
  #[must_use]
  pub fn drop(mut self, count: usize) -> Flow<In, Out, Mat> {
    let definition = drop_definition::<Out>(count);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a take stage that emits up to `count` elements.
  #[must_use]
  pub fn take(mut self, count: usize) -> Flow<In, Out, Mat> {
    let definition = take_definition::<Out>(count);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a drop-while stage to this flow.
  #[must_use]
  pub fn drop_while<F>(mut self, predicate: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = drop_while_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a take-while stage to this flow.
  #[must_use]
  pub fn take_while<F>(mut self, predicate: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = take_while_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a take-until stage to this flow.
  #[must_use]
  pub fn take_until<F>(mut self, predicate: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = take_until_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a grouped stage that emits vectors of size `size`.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` is zero.
  pub fn grouped(mut self, size: usize) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError> {
    let size = validate_positive_argument("size", size)?;
    let definition = grouped_definition::<Out>(size);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a sliding stage that emits windows with size `size`.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` is zero.
  pub fn sliding(mut self, size: usize) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
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
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a scan stage that emits running accumulation from `initial`.
  #[must_use]
  pub fn scan<Acc, F>(mut self, initial: Acc, func: F) -> Flow<In, Acc, Mat>
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds an intersperse stage with start, separator and end markers.
  #[must_use]
  pub fn intersperse(mut self, start: Out, inject: Out, end: Out) -> Flow<In, Out, Mat>
  where
    Out: Clone, {
    let definition = intersperse_definition::<Out>(start, inject, end);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a flatMapConcat stage to this flow.
  #[must_use]
  pub fn flat_map_concat<T, Mat2, F>(mut self, func: F) -> Flow<In, T, Mat>
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a flatMapMerge stage to this flow.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `breadth` is zero.
  pub fn flat_map_merge<T, Mat2, F>(mut self, breadth: usize, func: F) -> Result<Flow<In, T, Mat>, StreamDslError>
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
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
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
  ) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let capacity = validate_positive_argument("capacity", capacity)?;
    let definition = buffer_definition::<Out>(capacity, overflow_strategy);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds an explicit async boundary stage.
  #[must_use]
  pub fn async_boundary(mut self) -> Flow<In, Out, Mat> {
    let definition = async_boundary_definition::<Out>();
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a throttle stage that limits the number of buffered in-flight elements.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `capacity` is zero.
  pub fn throttle(mut self, capacity: usize, mode: super::ThrottleMode) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let capacity = validate_positive_argument("capacity", capacity)?;
    let definition = throttle_definition::<Out>(capacity, mode);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a delay stage that emits each element after `ticks`.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn delay(mut self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = delay_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds an initial-delay stage that suppresses outputs until `ticks` elapse.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn initial_delay(mut self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = initial_delay_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a debounce stage that emits the held element after `ticks` of silence.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn debounce(mut self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = debounce_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a sample stage that emits the latest element at fixed `ticks` intervals.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn sample(mut self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = sample_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a take-within stage that forwards elements only within `ticks`.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn take_within(mut self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = take_within_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a batch stage that emits vectors of size `size`.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` is zero.
  pub fn batch(mut self, size: usize) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError> {
    let size = validate_positive_argument("size", size)?;
    let definition = batch_definition::<Out>(size);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Enables restart semantics with backoff for this flow.
  #[must_use]
  pub fn restart_flow_with_backoff(mut self, min_backoff_ticks: u32, max_restarts: usize) -> Flow<In, Out, Mat> {
    self.graph.set_flow_restart(Some(RestartBackoff::new(min_backoff_ticks, max_restarts)));
    self
  }

  /// Compatibility alias for applying restart-on-failure backoff semantics.
  #[must_use]
  pub fn on_failures_with_backoff(self, min_backoff_ticks: u32, max_restarts: usize) -> Flow<In, Out, Mat> {
    self.restart_flow_with_backoff(min_backoff_ticks, max_restarts)
  }

  /// Compatibility alias for applying restart backoff semantics.
  #[must_use]
  pub fn with_backoff(self, min_backoff_ticks: u32, max_restarts: usize) -> Flow<In, Out, Mat> {
    self.restart_flow_with_backoff(min_backoff_ticks, max_restarts)
  }

  /// Compatibility alias for applying restart backoff semantics with ignored context parameter.
  #[must_use]
  pub fn with_backoff_and_context<C>(
    self,
    min_backoff_ticks: u32,
    max_restarts: usize,
    _context: C,
  ) -> Flow<In, Out, Mat> {
    self.restart_flow_with_backoff(min_backoff_ticks, max_restarts)
  }

  /// Enables restart semantics by explicit restart settings.
  #[must_use]
  pub fn restart_flow_with_settings(mut self, settings: RestartSettings) -> Flow<In, Out, Mat> {
    self.graph.set_flow_restart(Some(RestartBackoff::from_settings(settings)));
    self
  }

  /// Applies stop supervision semantics to this flow.
  #[must_use]
  pub fn supervision_stop(mut self) -> Flow<In, Out, Mat> {
    self.graph.set_flow_supervision(SupervisionStrategy::Stop);
    self
  }

  /// Applies resume supervision semantics to this flow.
  #[must_use]
  pub fn supervision_resume(mut self) -> Flow<In, Out, Mat> {
    self.graph.set_flow_supervision(SupervisionStrategy::Resume);
    self
  }

  /// Applies restart supervision semantics to this flow.
  #[must_use]
  pub fn supervision_restart(mut self) -> Flow<In, Out, Mat> {
    self.graph.set_flow_supervision(SupervisionStrategy::Restart);
    self
  }

  /// Adds a group-by stage and returns substream surface for merging grouped elements.
  ///
  /// Unsupported `SubFlow` operators, including `concat_substreams`, stay unavailable on the
  /// returned surface.
  ///
  /// ```compile_fail
  /// use fraktor_stream_rs::core::{StreamNotUsed, SubstreamCancelStrategy, stage::flow::Flow};
  ///
  /// let _ = Flow::<u32, u32, StreamNotUsed>::new()
  ///   .group_by(2, |value: &u32| value % 2, SubstreamCancelStrategy::default())
  ///   .expect("group_by")
  ///   .drop(1);
  /// ```
  ///
  /// ```compile_fail
  /// use fraktor_stream_rs::core::{StreamNotUsed, SubstreamCancelStrategy, stage::flow::Flow};
  ///
  /// let _ = Flow::<u32, u32, StreamNotUsed>::new()
  ///   .group_by(2, |value: &u32| value % 2, SubstreamCancelStrategy::default())
  ///   .expect("group_by")
  ///   .concat_substreams();
  /// ```
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `max_substreams` is zero.
  pub fn group_by<K, F>(
    mut self,
    max_substreams: usize,
    key_fn: F,
    cancel_strategy: SubstreamCancelStrategy,
  ) -> Result<FlowGroupBySubFlow<In, K, Out, Mat>, StreamDslError>
  where
    K: Clone + PartialEq + Send + Sync + 'static,
    F: FnMut(&Out) -> K + Send + Sync + 'static, {
    let max_substreams = validate_positive_argument("max_substreams", max_substreams)?;
    let definition = group_by_definition::<Out, K, F>(max_substreams, key_fn, cancel_strategy);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    let grouped = Flow::<In, (K, Out), Mat> { graph: self.graph, mat: self.mat, _pd: PhantomData };
    Ok(FlowGroupBySubFlow::from_flow(grouped))
  }

  /// Splits the stream before elements matching `predicate`.
  #[must_use]
  pub fn split_when<F>(mut self, predicate: F) -> FlowSubFlow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = split_when_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    FlowSubFlow::from_flow(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Splits the stream before elements matching `predicate` with explicit substream cancellation
  /// handling.
  #[must_use]
  pub fn split_when_with_cancel_strategy<F>(
    mut self,
    substream_cancel_strategy: SubstreamCancelStrategy,
    predicate: F,
  ) -> FlowSubFlow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = split_when_definition_with_cancel_strategy::<Out, F>(predicate, substream_cancel_strategy);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    FlowSubFlow::from_flow(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Splits the stream after elements matching `predicate`.
  #[must_use]
  pub fn split_after<F>(mut self, predicate: F) -> FlowSubFlow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = split_after_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    FlowSubFlow::from_flow(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Splits the stream after elements matching `predicate` with explicit substream cancellation
  /// handling.
  #[must_use]
  pub fn split_after_with_cancel_strategy<F>(
    mut self,
    substream_cancel_strategy: SubstreamCancelStrategy,
    predicate: F,
  ) -> FlowSubFlow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = split_after_definition_with_cancel_strategy::<Out, F>(predicate, substream_cancel_strategy);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    FlowSubFlow::from_flow(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a partition stage that routes each element to one of two output lanes.
  #[must_use]
  pub fn partition<F>(mut self, predicate: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = partition_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds an unzip-with stage that maps each element into a pair and routes them to two output
  /// lanes.
  #[must_use]
  pub fn unzip_with<T, F>(mut self, func: F) -> Flow<In, T, Mat>
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a broadcast stage that duplicates each element `fan_out` times.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_out` is zero.
  pub fn broadcast(mut self, fan_out: usize) -> Result<Flow<In, Out, Mat>, StreamDslError>
  where
    Out: Clone, {
    let _ = validate_positive_argument("fan_out", fan_out)?;
    let definition = broadcast_definition::<Out>(fan_out);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a balance stage that distributes elements across `fan_out` outputs.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_out` is zero.
  pub fn balance(mut self, fan_out: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let _ = validate_positive_argument("fan_out", fan_out)?;
    let definition = balance_definition::<Out>(fan_out);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a merge stage that merges `fan_in` upstream paths.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn merge(mut self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let _ = validate_positive_argument("fan_in", fan_in)?;
    let definition = merge_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds an interleave stage that consumes `fan_in` inputs in round-robin order.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn interleave(mut self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let _ = validate_positive_argument("fan_in", fan_in)?;
    let definition = interleave_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a prepend stage that prioritizes lower-index input lanes.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn prepend(mut self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let _ = validate_positive_argument("fan_in", fan_in)?;
    let definition = prepend_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a zip stage that emits one vector after receiving one element from each input.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn zip(mut self, fan_in: usize) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError> {
    let _ = validate_positive_argument("fan_in", fan_in)?;
    let definition = zip_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a zip-all stage that fills missing lanes with `fill_value` after completion.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn zip_all(mut self, fan_in: usize, fill_value: Out) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
  where
    Out: Clone, {
    let _ = validate_positive_argument("fan_in", fan_in)?;
    let definition = zip_all_definition::<Out>(fan_in, fill_value);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a zip-with-index stage that pairs each element with an incrementing index.
  #[must_use]
  pub fn zip_with_index(mut self) -> Flow<In, (Out, u64), Mat> {
    let definition = zip_with_index_definition::<Out>();
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a concat stage that emits all elements from each input in port order.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn concat(mut self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let _ = validate_positive_argument("fan_in", fan_in)?;
    let definition = concat_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Extracts the flow logics from this flow's graph, consuming it.
  ///
  /// Returns the extracted logics and the materialized value.
  pub(in crate::core) fn into_logics(self) -> (Vec<Box<dyn FlowLogic>>, Mat) {
    let (graph, mat) = self.into_parts();
    let stages = graph.into_stages();
    let logics = stages
      .into_iter()
      .filter_map(|s| if let StageDefinition::Flow(def) = s { Some(def.logic) } else { None })
      .collect();
    (logics, mat)
  }
}

impl<In, Out, Mat> Flow<In, Vec<Out>, Mat>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Merges split substreams into a single output stream.
  #[must_use]
  pub fn merge_substreams(mut self) -> Flow<In, Out, Mat> {
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Merges split substreams with an explicit parallelism value.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn merge_substreams_with_parallelism(mut self, parallelism: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
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
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Concatenates split substreams into a single output stream.
  #[must_use]
  pub fn concat_substreams(mut self) -> Flow<In, Out, Mat> {
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }
}

impl<In, Out, Mat> Flow<In, (Out, Out), Mat>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Adds an unzip stage that routes tuple components to two output lanes.
  #[must_use]
  pub fn unzip(mut self) -> Flow<In, Out, Mat> {
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }
}

impl<In, Out, Mat> Flow<In, Option<Out>, Mat>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Adds a flatten-optional stage to this flow.
  #[must_use]
  pub fn flatten_optional(self) -> Flow<In, Out, Mat> {
    self.map_option(|value| value)
  }
}

impl<In, Out, Mat, Mat2> Flow<In, Source<Out, Mat2>, Mat>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
{
  /// Flattens nested sources.
  #[must_use]
  pub fn flatten(mut self) -> Flow<In, Out, Mat> {
    let definition = flat_map_concat_definition::<Source<Out, Mat2>, Out, Mat2, _>(|source| source);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(
        &Outlet::<Source<Out, Mat2>>::from_id(from),
        &Inlet::<Source<Out, Mat2>>::from_id(inlet_id),
        MatCombine::KeepLeft,
      );
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }
}

impl<In, Out, Mat> Flow<In, Out, Mat>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Maps upstream failures into different stream failures.
  #[must_use]
  pub fn map_error<F>(mut self, mapper: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(StreamError) -> StreamError + Send + Sync + 'static, {
    let definition = map_error_definition::<Out, F>(mapper);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Resumes the stream when the upstream failure matches.
  #[must_use]
  pub fn on_error_continue(self) -> Flow<In, Out, Mat> {
    self.on_error_continue_if_with(|_| true, |_| {})
  }

  /// Resumes the stream and invokes `error_consumer` when the failure matches.
  #[must_use]
  pub fn on_error_continue_with<C>(self, error_consumer: C) -> Flow<In, Out, Mat>
  where
    C: FnMut(&StreamError) + Send + Sync + 'static, {
    self.on_error_continue_if_with(|_| true, error_consumer)
  }

  /// Resumes the stream when the upstream failure matches `predicate`.
  #[must_use]
  pub fn on_error_continue_if<P>(self, predicate: P) -> Flow<In, Out, Mat>
  where
    P: FnMut(&StreamError) -> bool + Send + Sync + 'static, {
    self.on_error_continue_if_with(predicate, |_| {})
  }

  /// Resumes the stream and invokes `error_consumer` when the failure matches `predicate`.
  #[must_use]
  pub fn on_error_continue_if_with<P, C>(mut self, predicate: P, error_consumer: C) -> Flow<In, Out, Mat>
  where
    P: FnMut(&StreamError) -> bool + Send + Sync + 'static,
    C: FnMut(&StreamError) + Send + Sync + 'static, {
    let definition = on_error_continue_definition::<Out, P, C>(predicate, error_consumer);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Alias of [`Flow::on_error_continue`].
  #[must_use]
  pub fn on_error_resume(self) -> Flow<In, Out, Mat> {
    self.on_error_continue()
  }

  /// Completes the stream when the upstream failure matches.
  #[must_use]
  pub fn on_error_complete(self) -> Flow<In, Out, Mat> {
    self.on_error_complete_if(|_| true)
  }

  /// Completes the stream when the upstream failure matches `predicate`.
  #[must_use]
  pub fn on_error_complete_if<P>(mut self, predicate: P) -> Flow<In, Out, Mat>
  where
    P: FnMut(&StreamError) -> bool + Send + Sync + 'static, {
    let definition = on_error_complete_definition::<Out, P>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Recovers an upstream failure with a single replacement element.
  #[must_use]
  pub fn recover<F>(mut self, recover: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(StreamError) -> Option<Out> + Send + Sync + 'static, {
    let definition = recover_definition::<Out, F>(recover);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Recovers upstream failures by switching to alternate sources.
  #[must_use]
  pub fn recover_with_retries<F>(mut self, max_retries: isize, recover: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(StreamError) -> Option<Source<Out, StreamNotUsed>> + Send + Sync + 'static, {
    let definition = recover_with_retries_definition::<Out, F>(max_retries, recover);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Alias of [`Flow::recover_with_retries`] with infinite retries.
  #[must_use]
  pub fn recover_with<F>(self, recover: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(StreamError) -> Option<Source<Out, StreamNotUsed>> + Send + Sync + 'static, {
    self.recover_with_retries(-1, recover)
  }
}

impl<In, Out, Mat> Flow<In, Out, Mat>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Wraps this flow with unit context propagation.
  #[must_use]
  pub fn as_flow_with_context(self) -> super::flow_with_context::FlowWithContext<(), In, Out, Mat> {
    let unwrap: Flow<((), In), In, StreamNotUsed> = Flow::from_function(|(_, value)| value);
    let rewrap: Flow<Out, ((), Out), StreamNotUsed> = Flow::from_function(|value| ((), value));
    let inner = unwrap.via_mat(self, super::keep_right::KeepRight).via(rewrap);
    super::flow_with_context::FlowWithContext::from_flow(inner)
  }

  /// Keeps only the first element matching `predicate`.
  #[must_use]
  pub fn collect_first<F>(self, predicate: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    self.filter(predicate).take(1)
  }

  /// Collects only values mapped to `Some`.
  #[must_use]
  pub fn collect<T, F>(self, func: F) -> Flow<In, T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> Option<T> + Send + Sync + 'static, {
    self.map_option(func)
  }

  /// Collects values that can be converted into `T`.
  #[must_use]
  pub fn collect_type<T>(self) -> Flow<In, T, Mat>
  where
    T: Send + Sync + 'static,
    Out: TryInto<T>, {
    self.map_option(|value| value.try_into().ok())
  }

  /// Keeps elements while `predicate` matches.
  #[must_use]
  pub fn collect_while<F>(self, predicate: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    self.take_while(predicate)
  }

  /// Compatibility alias for completion-stage flow entry points.
  #[must_use]
  pub const fn completion_stage_flow(self) -> Flow<In, Out, Mat> {
    self
  }

  /// Compatibility alias for contramap entry points.
  #[must_use]
  pub fn contramap<F>(self, _func: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&In) -> In + Send + Sync + 'static, {
    self
  }

  /// Decouples upstream and downstream demand signaling via an async boundary.
  #[must_use]
  pub fn detach(self) -> Flow<In, Out, Mat> {
    self.async_boundary()
  }

  /// Maps outputs while accepting dimap-compatible signatures.
  #[must_use]
  pub fn dimap<T, FL, FR>(self, _left: FL, right: FR) -> Flow<In, T, Mat>
  where
    T: Send + Sync + 'static,
    FL: Send + Sync + 'static,
    FR: FnMut(Out) -> T + Send + Sync + 'static, {
    self.map(right)
  }

  /// Registers a cancel callback placeholder.
  #[must_use]
  pub fn do_on_cancel<F>(self, _callback: F) -> Flow<In, Out, Mat>
  where
    F: FnMut() + Send + Sync + 'static, {
    self
  }

  /// Invokes a callback on the first element and passes all elements through unchanged.
  #[must_use]
  pub fn do_on_first<F>(self, mut callback: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) + Send + Sync + 'static, {
    let mut fired = false;
    self.wire_tap(move |value| {
      if !fired {
        fired = true;
        callback(value);
      }
    })
  }

  /// Folds all elements using an accumulator function, emitting the running accumulation.
  ///
  /// Unlike Pekko's `fold` which emits only the final value, this emits every
  /// intermediate accumulation (equivalent to `scan` without the initial value).
  #[must_use]
  pub fn fold<Acc, F>(self, initial: Acc, func: F) -> Flow<In, Acc, Mat>
  where
    Acc: Clone + Send + Sync + 'static,
    F: FnMut(Acc, Out) -> Acc + Send + Sync + 'static, {
    self.scan(initial, func).drop(1)
  }

  /// Reduces all elements using a binary function, emitting the running reduction.
  ///
  /// Uses the first element as the initial accumulator. Emits nothing if the
  /// stream is empty.
  #[must_use]
  pub fn reduce<F>(self, mut func: F) -> Flow<In, Out, Mat>
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

  /// Folds values asynchronously.
  ///
  /// This compatibility implementation updates the accumulator when the returned
  /// future resolves immediately.
  #[must_use]
  pub fn fold_async<Acc, F, Fut>(self, initial: Acc, func: F) -> Flow<In, Acc, Mat>
  where
    Acc: Clone + Send + Sync + 'static,
    F: FnMut(Acc, Out) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Acc> + Send + 'static, {
    self.stateful_map(move || {
      let mut acc = initial.clone();
      let mut func = func.clone();
      move |value| {
        let mut future = Box::pin((func)(acc.clone(), value));
        let waker = noop_waker();
        let mut cx = Context::from_waker(&waker);
        if let Poll::Ready(next) = future.as_mut().poll(&mut cx) {
          acc = next;
        }
        acc.clone()
      }
    })
  }

  /// Compatibility alias for future-flow entry points.
  #[must_use]
  pub const fn future_flow(self) -> Flow<In, Out, Mat> {
    self
  }

  /// Groups adjacent elements by key.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` is zero.
  pub fn grouped_adjacent_by<K, F>(self, size: usize, _key_fn: F) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
  where
    K: Send + Sync + 'static,
    F: FnMut(&Out) -> K + Send + Sync + 'static, {
    self.grouped(size)
  }

  /// Groups adjacent weighted elements by key.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` is zero.
  pub fn grouped_adjacent_by_weighted<K, FK, FW>(
    self,
    size: usize,
    _key_fn: FK,
    _weight_fn: FW,
  ) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
  where
    K: Send + Sync + 'static,
    FK: FnMut(&Out) -> K + Send + Sync + 'static,
    FW: FnMut(&Out) -> usize + Send + Sync + 'static, {
    self.grouped(size)
  }

  /// Groups weighted elements.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `max_weight` is zero.
  pub fn grouped_weighted<FW>(
    mut self,
    max_weight: usize,
    weight_fn: FW,
  ) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
  where
    FW: FnMut(&Out) -> usize + Send + Sync + 'static, {
    let max_weight = validate_positive_argument("max_weight", max_weight)?;
    let definition = grouped_weighted_definition::<Out, FW>(max_weight, weight_fn);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Lazily creates a completion-stage flow.
  ///
  /// Alias of [`Flow::lazy_flow`].
  #[must_use]
  pub fn lazy_completion_stage_flow<F>(factory: F) -> Flow<In, Out, Mat>
  where
    F: FnOnce() -> Flow<In, Out, Mat> + Send + 'static,
    Mat: Default + Send + 'static, {
    Self::lazy_flow(factory)
  }

  /// Lazily creates a flow.
  ///
  /// The factory is not called until the first element arrives.
  /// Flow stages from the created flow are extracted and chained for processing.
  ///
  /// The outer `Flow`'s Mat value uses `Mat::default()`. The factory-produced Mat
  /// is captured internally by `LazyFlowLogic` but is not propagated as the
  /// outer Mat.
  #[must_use]
  pub fn lazy_flow<F>(factory: F) -> Flow<In, Out, Mat>
  where
    F: FnOnce() -> Flow<In, Out, Mat> + Send + 'static,
    Mat: Default + Send + 'static, {
    let inlet: Inlet<In> = Inlet::new();
    let outlet: Outlet<Out> = Outlet::new();
    let logic = LazyFlowLogic::<In, Out, Mat, F> {
      factory:      Some(factory),
      inner_logics: Vec::new(),
      mat:          None,
      _pd:          PhantomData,
    };
    let definition = FlowDefinition {
      kind:        StageKind::Custom,
      inlet:       inlet.id(),
      outlet:      outlet.id(),
      input_type:  TypeId::of::<In>(),
      output_type: TypeId::of::<Out>(),
      mat_combine: MatCombine::KeepLeft,
      supervision: SupervisionStrategy::Stop,
      restart:     None,
      logic:       Box::new(logic),
    };
    let mut graph = StreamGraph::new();
    graph.push_stage(StageDefinition::Flow(definition));
    Flow::from_graph(graph, Mat::default())
  }

  /// Lazily creates a future flow.
  ///
  /// Alias of [`Flow::lazy_flow`].
  #[must_use]
  pub fn lazy_future_flow<F>(factory: F) -> Flow<In, Out, Mat>
  where
    F: FnOnce() -> Flow<In, Out, Mat> + Send + 'static,
    Mat: Default + Send + 'static, {
    Self::lazy_flow(factory)
  }

  /// Limits element count.
  #[must_use]
  pub fn limit(self, max: usize) -> Flow<In, Out, Mat> {
    self.take(max)
  }

  /// Limits weighted element count.
  #[must_use]
  pub fn limit_weighted<FW>(mut self, max_weight: usize, weight_fn: FW) -> Flow<In, Out, Mat>
  where
    FW: FnMut(&Out) -> usize + Send + Sync + 'static, {
    let definition = limit_weighted_definition::<Out, FW>(max_weight, weight_fn);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a logging stage and metadata while passing each element through unchanged.
  #[must_use]
  pub fn log(mut self, name: &'static str) -> Flow<In, Out, Mat> {
    let definition = log_definition::<Out>();
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }.add_attributes(Attributes::named(name))
  }

  /// Adds a marker-tagged logging stage and marker metadata while passing each element through
  /// unchanged.
  #[must_use]
  pub fn log_with_marker(self, name: &'static str, marker: &'static str) -> Flow<In, Out, Mat> {
    self.log(name).add_attributes(Attributes::named(marker))
  }

  /// Maps values with a mutable resource.
  #[must_use]
  pub fn map_with_resource<R, T, FR, FM>(self, mut resource_factory: FR, mapper: FM) -> Flow<In, T, Mat>
  where
    T: Send + Sync + 'static,
    R: Send + Sync + 'static,
    FR: FnMut() -> R + Send + Sync + 'static,
    FM: FnMut(&mut R, Out) -> T + Clone + Send + Sync + 'static, {
    self.stateful_map(move || {
      let mut resource = resource_factory();
      let mut mapper = mapper.clone();
      move |value| mapper(&mut resource, value)
    })
  }

  /// Materializes a source through this flow and emits the resolved sink materialized value.
  #[must_use]
  pub fn materialize_into_source<Mat1, Mat2>(
    self,
    source: Source<In, Mat1>,
    sink: Sink<Out, StreamCompletion<Mat2>>,
  ) -> Source<Mat2, StreamCompletion<()>>
  where
    Mat1: Send + Sync + 'static,
    Mat: Send + Sync + 'static,
    Mat2: Send + Sync + 'static, {
    let logic = MaterializeIntoSourceLogic::<Mat2, _> {
      factory:    Some(move || {
        let graph = source.via(self).to_mat(sink, KeepRight);
        let (plan, materialized) = graph.into_parts();
        let mut stream = Stream::new(plan, StreamBufferConfig::default());
        stream.start()?;
        Ok((stream, materialized))
      }),
      stream:     None,
      completion: None,
      emitted:    false,
      _pd:        PhantomData,
    };
    Source::from_logic(StageKind::Custom, logic).watch_termination_mat(KeepRight)
  }

  /// Optionally composes this flow with another flow.
  #[must_use]
  pub fn optional_via<Mat2>(self, flow: Option<Flow<Out, Out, Mat2>>) -> Flow<In, Out, Mat> {
    match flow {
      | Some(flow) => self.via(flow),
      | None => self,
    }
  }

  /// Compatibility alias for scan-async entry points.
  ///
  /// Only supports futures that resolve immediately (synchronously).
  /// Panics if the future returns `Pending`.
  #[must_use]
  pub fn scan_async<Acc, F, Fut>(self, initial: Acc, mut func: F) -> Flow<In, Acc, Mat>
  where
    Acc: Clone + Send + Sync + 'static,
    F: FnMut(Acc, Out) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Acc> + Send + 'static, {
    self.scan(initial, move |acc, value| {
      let mut future = Box::pin((func)(acc.clone(), value));
      let waker = noop_waker();
      let mut cx = Context::from_waker(&waker);
      match future.as_mut().poll(&mut cx) {
        | Poll::Ready(result) => result,
        | Poll::Pending => acc,
      }
    })
  }

  /// Compatibility alias for map-async unordered entry points.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn map_async_unordered<T, F, Fut>(self, parallelism: usize, func: F) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static, {
    self.map_async(parallelism, func)
  }

  /// Maps elements asynchronously while allowing at most one in-flight element per partition.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn map_async_partitioned<T, P, Partitioner, F, Fut>(
    mut self,
    parallelism: usize,
    partitioner: Partitioner,
    func: F,
  ) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    P: Clone + PartialEq + Send + Sync + 'static,
    Partitioner: FnMut(&Out) -> P + Send + Sync + 'static,
    F: FnMut(Out, P) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static, {
    let parallelism = validate_positive_argument("parallelism", parallelism)?;
    let definition =
      map_async_partitioned_definition::<Out, T, P, Partitioner, F, Fut>(parallelism, true, partitioner, func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Maps elements asynchronously with partition serialization and unordered downstream emission.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn map_async_partitioned_unordered<T, P, Partitioner, F, Fut>(
    mut self,
    parallelism: usize,
    partitioner: Partitioner,
    func: F,
  ) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    P: Clone + PartialEq + Send + Sync + 'static,
    Partitioner: FnMut(&Out) -> P + Send + Sync + 'static,
    F: FnMut(Out, P) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static, {
    let parallelism = validate_positive_argument("parallelism", parallelism)?;
    let definition =
      map_async_partitioned_definition::<Out, T, P, Partitioner, F, Fut>(parallelism, false, partitioner, func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Asks an actor-like endpoint asynchronously.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn ask<T, F, Fut>(self, parallelism: usize, func: F) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static, {
    self.map_async(parallelism, func)
  }

  /// Asks with status semantics.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn ask_with_status<T, F, Fut>(self, parallelism: usize, func: F) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static, {
    self.ask(parallelism, func)
  }

  /// Adds a per-element delay stage driven by a [`DelayStrategy`].
  ///
  /// Each element is delayed by the number of ticks returned by the
  /// strategy.  Unlike [`delay`](Self::delay), the delay can vary
  /// per element depending on the strategy implementation.
  ///
  /// [`DelayStrategy`]: crate::core::delay_strategy::DelayStrategy
  #[must_use]
  pub fn delay_with<S>(mut self, strategy: S) -> Flow<In, Out, Mat>
  where
    S: crate::core::delay_strategy::DelayStrategy<Out> + 'static, {
    let definition = strategy_delay_definition::<Out, S>(strategy);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Drops elements within a count-compatible window.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn drop_within(self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    Ok(self.drop(ticks))
  }

  /// Groups elements within a weighted window.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `max_weight` is zero.
  pub fn grouped_weighted_within<FW>(
    mut self,
    max_weight: usize,
    ticks: usize,
    weight_fn: FW,
  ) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
  where
    FW: FnMut(&Out) -> usize + Send + Sync + 'static, {
    let max_weight = validate_positive_argument("max_weight", max_weight)?;
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = grouped_weighted_within_definition::<Out, FW>(max_weight, ticks as u64, weight_fn);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Groups elements while tracking both group size and tick window progress.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` or `ticks` is zero.
  pub fn grouped_within(mut self, size: usize, ticks: usize) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError> {
    let size = validate_positive_argument("size", size)?;
    let ticks = validate_positive_argument("ticks", ticks)? as u64;
    let definition = grouped_within_definition::<Out>(size, ticks);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Aggregates elements with boundary semantics.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` is zero.
  pub fn aggregate_with_boundary(self, size: usize) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError> {
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
  ) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
  where
    FW: FnMut(&Out) -> usize + Send + Sync + 'static, {
    let max_weight = validate_positive_argument("max_weight", max_weight)?;
    let definition = grouped_weighted_definition::<Out, FW>(max_weight, weight_fn);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Conflates upstream elements by repeatedly aggregating all emitted values.
  #[must_use]
  pub fn conflate<FA>(self, aggregate: FA) -> Flow<In, Out, Mat>
  where
    Out: Send + Sync + 'static,
    FA: FnMut(Out, Out) -> Out + Send + Sync + 'static, {
    self.conflate_with_seed(|value| value, aggregate)
  }

  /// Adds a conflate-with-seed stage.
  #[must_use]
  pub fn conflate_with_seed<T, FS, FA>(mut self, seed: FS, aggregate: FA) -> Flow<In, T, Mat>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Expands each input element and extrapolates on idle ticks while upstream is active.
  #[must_use]
  pub fn expand<F, I>(mut self, expander: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> I + Send + Sync + 'static,
    I: IntoIterator<Item = Out> + 'static,
    <I as IntoIterator>::IntoIter: Send, {
    let definition = expand_definition::<Out, F, I>(expander);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Extrapolates elements with the same behavior as [`Flow::expand`].
  #[must_use]
  pub fn extrapolate<F, I>(self, expander: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> I + Send + Sync + 'static,
    I: IntoIterator<Item = Out> + 'static,
    <I as IntoIterator>::IntoIter: Send, {
    self.expand(expander)
  }

  /// Replaces graph attributes with the provided values.
  #[must_use]
  pub fn with_attributes(mut self, attributes: Attributes) -> Flow<In, Out, Mat> {
    self.graph.set_attributes(attributes);
    self
  }

  /// Appends graph attributes to the existing values.
  #[must_use]
  pub fn add_attributes(mut self, attributes: Attributes) -> Flow<In, Out, Mat> {
    self.graph.add_attributes(attributes);
    self
  }

  /// Assigns a debug name attribute to this stage graph.
  #[must_use]
  pub fn named(self, name: &str) -> Flow<In, Out, Mat> {
    self.add_attributes(Attributes::named(name))
  }

  /// Buffers a prefix before materializing a tail-processing flow.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when the stage graph cannot be attached to this flow.
  pub fn flat_map_prefix<T, Mat2, F>(mut self, prefix: usize, factory: F) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    Out: Send + Sync + 'static,
    T: Send + Sync + 'static,
    Mat2: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> Flow<Out, T, Mat2> + Send + Sync + 'static, {
    let definition = flat_map_prefix_definition::<Out, T, Mat2, F>(prefix, factory);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a flatten-merge compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `breadth` is zero.
  pub fn flatten_merge<T, Mat2, F>(self, breadth: usize, func: F) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    Mat2: Send + Sync + 'static,
    F: FnMut(Out) -> Source<T, Mat2> + Send + Sync + 'static, {
    self.flat_map_merge(breadth, func)
  }

  /// Emits a single `(prefix, tail)` tuple with the remaining elements exposed as a source.
  #[must_use]
  pub fn prefix_and_tail(mut self, size: usize) -> Flow<In, (Vec<Out>, TailSource<Out>), Mat> {
    let definition = prefix_and_tail_definition::<Out>(size);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a switch-map compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when switch-map configuration is invalid.
  pub fn switch_map<T, Mat2, F>(self, func: F) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    Mat2: Send + Sync + 'static,
    F: FnMut(Out) -> Source<T, Mat2> + Send + Sync + 'static, {
    self.flat_map_merge(1, func)
  }

  /// Fails the stream when downstream backpressure exceeds `ticks`.
  ///
  /// After the first element arrives, if no subsequent `apply` call occurs
  /// within `ticks` ticks, the stream fails with [`StreamError::Timeout`].
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn backpressure_timeout(mut self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = backpressure_timeout_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Fails the stream when it does not complete within `ticks`.
  ///
  /// The tick counter starts at stream start. If the stream has not
  /// completed by the time `tick_count` exceeds `ticks`, the stream
  /// fails with [`StreamError::Timeout`].
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn completion_timeout(mut self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = completion_timeout_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Fails the stream when no element arrives within `ticks`.
  ///
  /// The tick counter starts at stream start and resets on every element.
  /// If the gap between successive elements (or between start and the
  /// first element) exceeds `ticks`, the stream fails with
  /// [`StreamError::Timeout`].
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn idle_timeout(mut self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = idle_timeout_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Fails the stream when the first element does not arrive within `ticks`.
  ///
  /// If `tick_count` exceeds `ticks` before the first `apply` call, the
  /// stream fails with [`StreamError::Timeout`]. Once the first element
  /// arrives, this stage becomes a pure pass-through.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn initial_timeout(mut self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let ticks = validate_positive_argument("ticks", ticks)?;
    let definition = initial_timeout_definition::<Out>(ticks as u64);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Applies a keep-alive compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn keep_alive(self, ticks: usize, value: Out) -> Result<Flow<In, Out, Mat>, StreamDslError>
  where
    Out: Clone, {
    let _ = validate_positive_argument("ticks", ticks)?;
    Ok(self.intersperse(value.clone(), value.clone(), value))
  }

  /// Adds a merge-sequence compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn merge_sequence(self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.merge(fan_in)
  }

  /// Adds a concat-all-lazy compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn concat_all_lazy(self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.concat(fan_in)
  }

  /// Concatenates a secondary source after the primary flow completes.
  #[must_use]
  pub fn concat_lazy<Mat2>(mut self, source: Source<Out, Mat2>) -> Flow<In, Out, Mat>
  where
    Mat2: Send + Sync + 'static, {
    let definition = concat_lazy_definition::<Out, Mat2>(source);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Concatenates a secondary source after the primary flow completes and combines materialized
  /// values.
  #[must_use]
  pub fn concat_lazy_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Flow<In, Out, C::Out>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Zip stage that combines materialized values.
  #[must_use]
  pub fn zip_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Flow<In, Vec<Out>, C::Out>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    if let Some(src_out) = source_tail {
      let _ =
        self.graph.connect(&Outlet::<Out>::from_id(src_out), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepRight);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Zip-all stage that combines materialized values.
  #[must_use]
  pub fn zip_all_mat<Mat2, C>(
    mut self,
    source: Source<Out, Mat2>,
    fill_value: Out,
    _combine: C,
  ) -> Flow<In, Vec<Out>, C::Out>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    if let Some(src_out) = source_tail {
      let _ =
        self.graph.connect(&Outlet::<Out>::from_id(src_out), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepRight);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Zip-with stage that combines materialized values.
  #[must_use]
  pub fn zip_with_mat<T, Mat2, F, C>(self, source: Source<Out, Mat2>, func: F, combine: C) -> Flow<In, T, C::Out>
  where
    T: Send + Sync + 'static,
    Mat2: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> T + Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    self.zip_mat(source, combine).map(func)
  }

  /// Zip-latest stage that combines materialized values.
  #[must_use]
  pub fn zip_latest_mat<Mat2, C>(self, source: Source<Out, Mat2>, combine: C) -> Flow<In, Vec<Out>, C::Out>
  where
    Out: Clone,
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    self.merge_latest_mat(source, combine)
  }

  /// Zip-latest-with stage that combines materialized values.
  #[must_use]
  pub fn zip_latest_with_mat<T, Mat2, F, C>(
    self,
    source: Source<Out, Mat2>,
    func: F,
    combine: C,
  ) -> Flow<In, T, C::Out>
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
  pub fn merge_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Flow<In, Out, C::Out>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    if let Some(src_out) = source_tail {
      let _ =
        self.graph.connect(&Outlet::<Out>::from_id(src_out), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepRight);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Merge-latest stage that combines materialized values.
  #[must_use]
  pub fn merge_latest_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Flow<In, Vec<Out>, C::Out>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    if let Some(src_out) = source_tail {
      let _ =
        self.graph.connect(&Outlet::<Out>::from_id(src_out), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepRight);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Merge-preferred stage that combines materialized values.
  #[must_use]
  pub fn merge_preferred_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Flow<In, Out, C::Out>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    if let Some(src_out) = source_tail {
      let _ =
        self.graph.connect(&Outlet::<Out>::from_id(src_out), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepRight);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Merge-prioritized stage that combines materialized values.
  #[must_use]
  pub fn merge_prioritized_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Flow<In, Out, C::Out>
  where
    Mat2: Send + Sync + 'static,
    C: MatCombineRule<Mat, Mat2>, {
    let (source_graph, right_mat) = source.into_parts();
    let source_tail = source_graph.tail_outlet();
    let from = self.graph.tail_outlet();
    self.graph.append_unwired(source_graph);
    let equal_priorities: Vec<usize> = vec![1; 2];
    let definition = merge_prioritized_definition::<Out>(2, &equal_priorities);
    let inlet_id = definition.inlet;
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    if let Some(src_out) = source_tail {
      let _ =
        self.graph.connect(&Outlet::<Out>::from_id(src_out), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepRight);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Concat stage that combines materialized values.
  #[must_use]
  pub fn concat_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Flow<In, Out, C::Out>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Prepend stage that combines materialized values.
  #[must_use]
  pub fn prepend_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Flow<In, Out, C::Out>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Interleave stage that combines materialized values.
  #[must_use]
  pub fn interleave_mat<Mat2, C>(
    mut self,
    source: Source<Out, Mat2>,
    _segment_size: usize,
    _combine: C,
  ) -> Flow<In, Out, C::Out>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    if let Some(src_out) = source_tail {
      let _ =
        self.graph.connect(&Outlet::<Out>::from_id(src_out), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepRight);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Flat-map-prefix stage that combines materialized values.
  #[must_use]
  pub fn flat_map_prefix_mat<T, Mat2, F, C>(
    mut self,
    prefix: usize,
    mut factory: F,
    _combine: C,
  ) -> Flow<In, T, C::Out>
  where
    Out: Send + Sync + 'static,
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Adds an interleave-all compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn interleave_all(self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.interleave(fan_in)
  }

  /// Adds a merge-all compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn merge_all(self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.merge(fan_in)
  }

  /// Keeps the latest value from each of `fan_in` input ports and emits a
  /// `Vec<Out>` snapshot every time any input is updated.
  ///
  /// No output is produced until every input has delivered at least one
  /// element.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn merge_latest(mut self, fan_in: usize) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
  where
    Out: Clone, {
    let _ = validate_positive_argument("fan_in", fan_in)?;
    let definition = merge_latest_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a merge-preferred stage that prioritizes slot 0 (preferred) input.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn merge_preferred(mut self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let _ = validate_positive_argument("fan_in", fan_in)?;
    let definition = merge_preferred_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a merge-prioritized stage with equal weights across all input ports.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn merge_prioritized(mut self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let _ = validate_positive_argument("fan_in", fan_in)?;
    let equal_priorities: Vec<usize> = vec![1; fan_in];
    let definition = merge_prioritized_definition::<Out>(fan_in, &equal_priorities);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Adds a merge-prioritized stage with custom weight priorities per input port.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero, when
  /// `priorities.len() != fan_in`, or when any priority is zero.
  pub fn merge_prioritized_n(
    mut self,
    fan_in: usize,
    priorities: &[usize],
  ) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let _ = validate_positive_argument("fan_in", fan_in)?;
    if priorities.len() != fan_in {
      return Err(StreamDslError::InvalidArgument {
        name:   "priorities",
        value:  priorities.len(),
        reason: "length must match fan_in",
      });
    }
    for (i, &p) in priorities.iter().enumerate() {
      if p == 0 {
        return Err(StreamDslError::InvalidArgument {
          name:   "priorities",
          value:  i,
          reason: "all priorities must be positive",
        });
      }
    }
    let definition = merge_prioritized_definition::<Out>(fan_in, priorities);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Falls back to a secondary source when the primary flow emits no elements.
  #[must_use]
  pub fn or_else<Mat2>(mut self, secondary: Source<Out, Mat2>) -> Flow<In, Out, Mat>
  where
    Mat2: Send + Sync + 'static, {
    let definition = or_else_definition::<Out, Mat2>(secondary);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Falls back to a secondary source when the primary flow emits no elements and combines
  /// materialized values.
  #[must_use]
  pub fn or_else_mat<Mat2, C>(mut self, secondary: Source<Out, Mat2>, _combine: C) -> Flow<In, Out, C::Out>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Prepends a secondary source before the primary flow starts emitting.
  #[must_use]
  pub fn prepend_lazy<Mat2>(mut self, source: Source<Out, Mat2>) -> Flow<In, Out, Mat>
  where
    Mat2: Send + Sync + 'static, {
    let definition = prepend_lazy_definition::<Out, Mat2>(source);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Prepends a secondary source before the primary flow starts emitting and combines materialized
  /// values.
  #[must_use]
  pub fn prepend_lazy_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Flow<In, Out, C::Out>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Adds a zip-latest compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn zip_latest(self, fan_in: usize) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
  where
    Out: Clone, {
    self.merge_latest(fan_in)
  }

  /// Adds a zip-latest-with compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn zip_latest_with<T, F>(self, fan_in: usize, func: F) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    Out: Clone,
    T: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> T + Send + Sync + 'static, {
    Ok(self.zip_latest(fan_in)?.map(func))
  }

  /// Adds a zip-with compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn zip_with<T, F>(self, fan_in: usize, func: F) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> T + Send + Sync + 'static, {
    Ok(self.zip(fan_in)?.map(func))
  }

  /// Adds an also-to compatibility stage.
  #[must_use]
  pub fn also_to<Mat2>(self, sink: Sink<Out, Mat2>) -> Flow<In, Out, Mat>
  where
    Out: Clone, {
    self.also_to_mat(sink, super::keep_left::KeepLeft)
  }

  /// Adds an also-to stage and combines materialized values.
  #[must_use]
  pub fn also_to_mat<Mat2, C>(self, sink: Sink<Out, Mat2>, _combine: C) -> Flow<In, Out, C::Out>
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
      let _ = graph.connect(
        &Outlet::<Out>::from_id(upstream_outlet),
        &Inlet::<Out>::from_id(broadcast_inlet),
        MatCombine::KeepLeft,
      );
    }
    let passthrough = map_definition::<Out, Out, _>(|value| value);
    let passthrough_inlet = passthrough.inlet;
    sink_graph.push_stage(StageDefinition::Flow(passthrough));
    graph.append(sink_graph);
    let _ = graph.connect(
      &Outlet::<Out>::from_id(broadcast_outlet),
      &Inlet::<Out>::from_id(passthrough_inlet),
      MatCombine::KeepLeft,
    );
    let mat = combine_mat::<Mat, Mat2, C>(left_mat, right_mat);
    Flow::from_graph(graph, mat)
  }

  /// Adds an also-to-all compatibility stage.
  #[must_use]
  pub fn also_to_all<Mat2, I>(self, sinks: I) -> Flow<In, Out, Mat>
  where
    I: IntoIterator<Item = Sink<Out, Mat2>>, {
    let _ = sinks.into_iter().count();
    self
  }

  /// Adds a divert-to stage that routes elements matching the predicate to a sink.
  ///
  /// Elements matching `predicate` are sent to `sink`; non-matching elements
  /// continue downstream.
  #[must_use]
  pub fn divert_to<Mat2, F>(self, predicate: F, sink: Sink<Out, Mat2>) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    self.divert_to_mat(predicate, sink, super::keep_left::KeepLeft)
  }

  /// Adds a divert-to stage and combines materialized values.
  ///
  /// Elements matching `predicate` are sent to `sink`; non-matching elements
  /// continue downstream.
  #[must_use]
  pub fn divert_to_mat<Mat2, F, C>(self, predicate: F, sink: Sink<Out, Mat2>, _combine: C) -> Flow<In, Out, C::Out>
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
      let _ = graph.connect(
        &Outlet::<Out>::from_id(upstream_outlet),
        &Inlet::<Out>::from_id(partition_inlet),
        MatCombine::KeepLeft,
      );
    }
    let passthrough = map_definition::<Out, Out, _>(|value| value);
    let passthrough_inlet = passthrough.inlet;
    sink_graph.push_stage(StageDefinition::Flow(passthrough));
    graph.append(sink_graph);
    let _ = graph.connect(
      &Outlet::<Out>::from_id(partition_outlet),
      &Inlet::<Out>::from_id(passthrough_inlet),
      MatCombine::KeepLeft,
    );
    let mat = combine_mat::<Mat, Mat2, C>(left_mat, right_mat);
    Flow::from_graph(graph, mat)
  }

  /// Adds a wire-tap compatibility stage.
  #[must_use]
  pub fn wire_tap<F>(self, mut callback: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) + Send + Sync + 'static, {
    self.map(move |value| {
      callback(&value);
      value
    })
  }

  /// Adds a wire-tap stage and combines materialized values.
  #[must_use]
  pub fn wire_tap_mat<Mat2, C>(self, sink: Sink<Out, Mat2>, combine: C) -> Flow<In, Out, C::Out>
  where
    Out: Clone,
    C: MatCombineRule<Mat, Mat2>, {
    self.also_to_mat(sink, combine)
  }

  /// Adds an actor-watch compatibility stage.
  #[must_use]
  pub const fn watch(self) -> Flow<In, Out, Mat> {
    self
  }

  /// Adds a monitor compatibility stage.
  #[must_use]
  pub fn monitor(self) -> Flow<In, (u64, Out), Mat> {
    self.zip_with_index().map(|(value, index)| (index, value))
  }

  /// Adds a monitor compatibility stage and combines materialized values.
  #[must_use]
  pub fn monitor_mat<C>(self, _combine: C) -> Flow<In, Out, C::Out>
  where
    C: MatCombineRule<Mat, FlowMonitor<Out>>, {
    let (graph, left_mat) = self.into_parts();
    let mat = combine_mat::<Mat, FlowMonitor<Out>, C>(left_mat, FlowMonitor::<Out>::new());
    Flow::from_graph(graph, mat)
  }

  /// Watches stream termination and completes a `StreamCompletion<()>` handle.
  ///
  /// Elements are passed through unchanged. The materialized value is
  /// combined with a fresh `StreamCompletion<()>` using the supplied
  /// `MatCombineRule`.
  #[must_use]
  pub fn watch_termination_mat<C>(mut self, _combine: C) -> Flow<In, Out, C::Out>
  where
    C: MatCombineRule<Mat, super::StreamCompletion<()>>, {
    let completion = super::StreamCompletion::<()>::new();
    let definition = watch_termination_definition::<Out>(completion.clone());
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    let mat = combine_mat::<Mat, super::StreamCompletion<()>, C>(self.mat, completion);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }

  /// Adds a deflate compatibility stage.
  #[cfg(feature = "compression")]
  #[must_use]
  pub fn deflate(mut self) -> Flow<In, Out, Mat>
  where
    Out: AsRef<[u8]> + From<Vec<u8>>, {
    let definition =
      try_map_concat_definition::<Out, Out, _>(|value| Ok(vec![Out::from(deflate_bytes(value.as_ref()))]));
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }.add_attributes(Attributes::named("compression:deflate"))
  }

  /// Adds a gzip compatibility stage.
  #[cfg(feature = "compression")]
  #[must_use]
  pub fn gzip(mut self) -> Flow<In, Out, Mat>
  where
    Out: AsRef<[u8]> + From<Vec<u8>>, {
    let definition = try_map_concat_definition::<Out, Out, _>(|value| Ok(vec![Out::from(gzip_bytes(value.as_ref()))]));
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }.add_attributes(Attributes::named("compression:gzip"))
  }

  /// Adds a gzip-decompress compatibility stage.
  #[cfg(feature = "compression")]
  #[must_use]
  pub fn gzip_decompress(mut self) -> Flow<In, Out, Mat>
  where
    Out: AsRef<[u8]> + From<Vec<u8>>, {
    let definition =
      try_map_concat_definition::<Out, Out, _>(|value| Ok(vec![Out::from(gunzip_bytes(value.as_ref())?)]));
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
      .add_attributes(Attributes::named("compression:gzip_decompress"))
  }

  /// Adds an inflate compatibility stage.
  #[cfg(feature = "compression")]
  #[must_use]
  pub fn inflate(mut self) -> Flow<In, Out, Mat>
  where
    Out: AsRef<[u8]> + From<Vec<u8>>, {
    let definition =
      try_map_concat_definition::<Out, Out, _>(|value| Ok(vec![Out::from(inflate_bytes(value.as_ref())?)]));
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }.add_attributes(Attributes::named("compression:inflate"))
  }
}

impl<In, Ctx, Req, Mat> Flow<In, (Ctx, Req), Mat>
where
  In: Send + Sync + 'static,
  Ctx: Send + Sync + 'static,
  Req: Send + Sync + 'static,
{
  /// Asks while preserving a context value.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn ask_with_context<T, F, Fut>(
    self,
    parallelism: usize,
    mut func: F,
  ) -> Result<Flow<In, (Ctx, T), Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    F: FnMut(Req) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static, {
    self.map_async(parallelism, move |(ctx, request)| {
      let future = (func)(request);
      async move { (ctx, future.await) }
    })
  }

  /// Asks with status semantics while preserving a context value.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn ask_with_status_and_context<T, F, Fut>(
    self,
    parallelism: usize,
    func: F,
  ) -> Result<Flow<In, (Ctx, T), Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    F: FnMut(Req) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static, {
    self.ask_with_context(parallelism, func)
  }
}

impl<In, Out, Mat> Flow<In, Out, Mat>
where
  In: Send + Sync + 'static,
  Out: Clone + PartialEq + Send + Sync + 'static,
{
  /// Drops repeated consecutive elements.
  #[must_use]
  pub fn drop_repeated(self) -> Flow<In, Out, Mat> {
    self
      .stateful_map(|| {
        let mut last: Option<Out> = None;
        move |value| {
          if last.as_ref().is_some_and(|current| current == &value) {
            return None;
          }
          last = Some(value.clone());
          Some(value)
        }
      })
      .flatten_optional()
  }
}

impl<In, Out, Mat> Flow<In, Out, Mat>
where
  In: Send + Sync + 'static,
  Out: Clone + Ord + Send + Sync + 'static,
{
  /// Filters out elements that have already been seen, using `Ord` for tracking.
  #[must_use]
  pub fn distinct(self) -> Flow<In, Out, Mat> {
    self
      .stateful_map(|| {
        let mut seen = alloc::collections::BTreeSet::new();
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

impl<In, Out, Mat> Flow<In, Out, Mat>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Filters out elements whose key has already been seen.
  #[must_use]
  pub fn distinct_by<K, F>(self, key_fn: F) -> Flow<In, Out, Mat>
  where
    K: Ord + Send + Sync + 'static,
    F: FnMut(&Out) -> K + Clone + Send + Sync + 'static, {
    self
      .stateful_map(move || {
        let mut key_fn = key_fn.clone();
        let mut seen = alloc::collections::BTreeSet::<K>::new();
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

impl<In, Out, Mat> Flow<In, Out, Mat>
where
  In: Send + Sync + 'static,
  Out: Ord + Send + Sync + 'static,
{
  /// Adds a merge-sorted stage that merges pre-sorted inputs into a single sorted output.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn merge_sorted(mut self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let _ = validate_positive_argument("fan_in", fan_in)?;
    let definition = merge_sorted_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Ok(Flow { graph: self.graph, mat: self.mat, _pd: PhantomData })
  }

  /// Merge-sorted stage that combines materialized values.
  #[must_use]
  pub fn merge_sorted_mat<Mat2, C>(mut self, source: Source<Out, Mat2>, _combine: C) -> Flow<In, Out, C::Out>
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
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    if let Some(src_out) = source_tail {
      let _ =
        self.graph.connect(&Outlet::<Out>::from_id(src_out), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepRight);
    }
    let mat = combine_mat::<Mat, Mat2, C>(self.mat, right_mat);
    Flow { graph: self.graph, mat, _pd: PhantomData }
  }
}

impl<In, Out> Flow<In, Out, StreamNotUsed>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Creates a flow from a sink (inlet side) and a source (outlet side).
  ///
  /// The resulting flow accepts elements into `sink` and emits elements from
  /// `source`. The two are composed into a single graph.
  #[must_use]
  pub fn from_sink_and_source<Mat1, Mat2>(sink: Sink<In, Mat1>, source: Source<Out, Mat2>) -> Self {
    let (mut sink_graph, _sink_mat) = sink.into_parts();
    let (source_graph, _source_mat) = source.into_parts();
    sink_graph.append_unwired(source_graph);
    Self { graph: sink_graph, mat: StreamNotUsed::new(), _pd: PhantomData }
  }

  /// Creates a flow from a sink and a source, combining materialized values.
  #[must_use]
  pub fn from_sink_and_source_mat<Mat1, Mat2, C>(
    sink: Sink<In, Mat1>,
    source: Source<Out, Mat2>,
    _combine: C,
  ) -> Flow<In, Out, C::Out>
  where
    C: MatCombineRule<Mat1, Mat2>, {
    let (mut sink_graph, left_mat) = sink.into_parts();
    let (source_graph, right_mat) = source.into_parts();
    sink_graph.append_unwired(source_graph);
    let mat = combine_mat::<Mat1, Mat2, C>(left_mat, right_mat);
    Flow::from_graph(sink_graph, mat)
  }

  /// Creates a coupled flow from a sink and a source.
  #[must_use]
  pub fn from_sink_and_source_coupled<Mat1, Mat2>(sink: Sink<In, Mat1>, source: Source<Out, Mat2>) -> Self {
    Self::from_sink_and_source(sink, source)
  }

  /// Creates a coupled flow from a sink and a source with materialized value control.
  #[must_use]
  pub fn from_sink_and_source_coupled_mat<Mat1, Mat2, C>(
    sink: Sink<In, Mat1>,
    source: Source<Out, Mat2>,
    combine: C,
  ) -> Flow<In, Out, C::Out>
  where
    C: MatCombineRule<Mat1, Mat2>, {
    Self::from_sink_and_source_mat(sink, source, combine)
  }
}

impl<In, Out, Mat> StreamStage for Flow<In, Out, Mat> {
  type In = In;
  type Out = Out;

  fn shape(&self) -> StreamShape<Self::In, Self::Out> {
    let inlet = self.graph.head_inlet().map(Inlet::from_id).unwrap_or_default();
    let outlet = self.graph.tail_outlet().map(Outlet::from_id).unwrap_or_default();
    StreamShape::new(inlet, outlet)
  }
}

pub(in crate::core) fn map_definition<In, Out, F>(func: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + 'static,
  F: FnMut(In) -> Out + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = MapLogic { func, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn map_async_definition<In, Out, F, Fut>(parallelism: usize, func: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Fut + Send + Sync + 'static,
  Fut: Future<Output = Out> + Send + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = MapAsyncLogic::<In, Out, F, Fut> { func, parallelism, pending: VecDeque::new(), _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowMapAsync,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn map_async_partitioned_definition<In, Out, P, Partitioner, F, Fut>(
  parallelism: usize,
  ordered: bool,
  partitioner: Partitioner,
  func: F,
) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  P: Clone + PartialEq + Send + Sync + 'static,
  Partitioner: FnMut(&In) -> P + Send + Sync + 'static,
  F: FnMut(In, P) -> Fut + Send + Sync + 'static,
  Fut: Future<Output = Out> + Send + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = MapAsyncPartitionedLogic::<In, Out, P, Partitioner, F, Fut>::new(partitioner, func, parallelism, ordered);
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

fn log_definition<In>() -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = LogLogic::<In>::new();
  FlowDefinition {
    kind:        StageKind::FlowLog,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn stateful_map_definition<In, Out, Factory, Mapper>(factory: Factory) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Factory: FnMut() -> Mapper + Send + Sync + 'static,
  Mapper: FnMut(In) -> Out + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let mut factory = factory;
  let mapper = factory();
  let logic = StatefulMapLogic::<In, Out, Factory, Mapper> { factory, mapper, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowStatefulMap,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn stateful_map_concat_definition<In, Out, Factory, Mapper, I>(factory: Factory) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Factory: FnMut() -> Mapper + Send + Sync + 'static,
  Mapper: FnMut(In) -> I + Send + Sync + 'static,
  I: IntoIterator<Item = Out> + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let mut factory = factory;
  let mapper = factory();
  let logic = StatefulMapConcatLogic::<In, Out, Factory, Mapper, I> { factory, mapper, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowStatefulMapConcat,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn map_concat_definition<In, Out, F, I>(func: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> I + Send + Sync + 'static,
  I: IntoIterator<Item = Out> + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = MapConcatLogic::<In, Out, F, I> { func, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowMapConcat,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn map_option_definition<In, Out, F>(func: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Option<Out> + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = MapOptionLogic::<In, Out, F> { func, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowMapOption,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

#[cfg(feature = "compression")]
pub(in crate::core) fn try_map_concat_definition<In, Out, F>(func: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Result<Vec<Out>, StreamError> + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = TryMapConcatLogic::<In, Out, F> { func, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn filter_definition<In, F>(predicate: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = FilterLogic::<In, F> { predicate, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowFilter,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn drop_definition<In>(count: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = DropLogic::<In> { remaining: count, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowDrop,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn take_definition<In>(count: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = TakeLogic::<In> { remaining: count, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowTake,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn limit_weighted_definition<In, FW>(max_weight: usize, weight_fn: FW) -> FlowDefinition
where
  In: Send + Sync + 'static,
  FW: FnMut(&In) -> usize + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic =
    LimitWeightedLogic::<In, FW> { remaining: max_weight, weight_fn, shutdown_requested: false, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn drop_while_definition<In, F>(predicate: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = DropWhileLogic::<In, F> { predicate, dropping: true, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowDropWhile,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn take_while_definition<In, F>(predicate: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = TakeWhileLogic::<In, F> { predicate, taking: true, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowTakeWhile,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn take_until_definition<In, F>(predicate: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = TakeUntilLogic::<In, F> { predicate, taking: true, shutdown_requested: false, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowTakeUntil,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn grouped_definition<In>(size: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = GroupedLogic::<In> { size, current: Vec::new(), source_done: false };
  FlowDefinition {
    kind:        StageKind::FlowGrouped,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Vec<In>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn grouped_within_definition<In>(size: usize, duration_ticks: u64) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = GroupedWithinLogic::<In> {
    size,
    duration_ticks,
    tick_count: 0,
    window_start_tick: None,
    current: Vec::new(),
    pending: VecDeque::new(),
  };
  FlowDefinition {
    kind:        StageKind::FlowStatefulMapConcat,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Vec<In>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn grouped_weighted_definition<In, FW>(max_weight: usize, weight_fn: FW) -> FlowDefinition
where
  In: Send + Sync + 'static,
  FW: FnMut(&In) -> usize + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = GroupedWeightedWithinLogic::<In, FW> {
    max_weight,
    duration_ticks: None,
    tick_count: 0,
    window_start_tick: None,
    current: Vec::new(),
    current_weight: 0,
    pending: VecDeque::new(),
    weight_fn,
  };
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Vec<In>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn grouped_weighted_within_definition<In, FW>(
  max_weight: usize,
  duration_ticks: u64,
  weight_fn: FW,
) -> FlowDefinition
where
  In: Send + Sync + 'static,
  FW: FnMut(&In) -> usize + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = GroupedWeightedWithinLogic::<In, FW> {
    max_weight,
    duration_ticks: Some(duration_ticks),
    tick_count: 0,
    window_start_tick: None,
    current: Vec::new(),
    current_weight: 0,
    pending: VecDeque::new(),
    weight_fn,
  };
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Vec<In>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn conflate_with_seed_definition<In, T, FS, FA>(seed: FS, aggregate: FA) -> FlowDefinition
where
  In: Send + Sync + 'static,
  T: Send + Sync + 'static,
  FS: FnMut(In) -> T + Send + Sync + 'static,
  FA: FnMut(T, In) -> T + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<T> = Outlet::new();
  let logic = ConflateWithSeedLogic::<In, T, FS, FA> {
    seed,
    aggregate,
    pending: None,
    just_updated: false,
    _pd: core::marker::PhantomData,
  };
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<T>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn expand_definition<In, F, I>(expander: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> I + Send + Sync + 'static,
  I: IntoIterator<Item = In> + 'static,
  <I as IntoIterator>::IntoIter: Send, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = ExpandLogic::<In, F> {
    expander,
    last: None,
    pending: None,
    tick_count: 0,
    last_input_tick: None,
    last_extrapolation_tick: None,
    source_done: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowStatefulMapConcat,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn sliding_definition<In>(size: usize) -> FlowDefinition
where
  In: Clone + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = SlidingLogic::<In> { size, window: VecDeque::new() };
  FlowDefinition {
    kind:        StageKind::FlowSliding,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Vec<In>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn scan_definition<In, Acc, F>(initial: Acc, func: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Acc: Clone + Send + Sync + 'static,
  F: FnMut(Acc, In) -> Acc + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Acc> = Outlet::new();
  let logic = ScanLogic::<In, Acc, F> {
    initial: initial.clone(),
    current: initial,
    func,
    initial_emitted: false,
    source_done: false,
    _pd: PhantomData,
  };
  FlowDefinition {
    kind:        StageKind::FlowScan,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Acc>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn intersperse_definition<In>(start: In, inject: In, end: In) -> FlowDefinition
where
  In: Clone + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = IntersperseLogic::<In> {
    start,
    inject,
    end,
    pending: VecDeque::new(),
    needs_start: true,
    seen_value: false,
    source_done: false,
    end_emitted: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowIntersperse,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn flat_map_concat_definition<In, Out, Mat2, F>(func: F) -> FlowDefinition
where
  In: Send + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = FlatMapConcatLogic { func, active_inner: None, pending_outer: VecDeque::new(), _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowFlatMapConcat,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn flat_map_merge_definition<In, Out, Mat2, F>(breadth: usize, func: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = FlatMapMergeLogic {
    breadth,
    func,
    active_streams: VecDeque::new(),
    pending_outer: VecDeque::new(),
    _pd: PhantomData,
  };
  FlowDefinition {
    kind:        StageKind::FlowFlatMapMerge,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn flat_map_prefix_definition<In, Out, Mat2, F>(prefix_len: usize, factory: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(Vec<In>) -> Flow<In, Out, Mat2> + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = FlatMapPrefixLogic::<In, Out, Mat2, F> {
    prefix_len,
    factory,
    prefix_values: Vec::new(),
    inner_logics: Vec::new(),
    factory_built: false,
    source_done: false,
    _pd: PhantomData,
  };
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn prefix_and_tail_definition<In>(prefix_len: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<(Vec<In>, TailSource<In>)> = Outlet::new();
  let logic = PrefixAndTailLogic::<In>::new(prefix_len);
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<(Vec<In>, TailSource<In>)>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn buffer_definition<In>(capacity: usize, overflow_strategy: OverflowStrategy) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = BufferLogic::<In> {
    capacity,
    overflow_mode: BufferOverflowMode::Strategy(overflow_strategy),
    pending: VecDeque::new(),
    source_done: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowBuffer,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn async_boundary_definition<In>() -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = AsyncBoundaryLogic::<In> { pending: VecDeque::new(), capacity: 16, enforcing: false };
  FlowDefinition {
    kind:        StageKind::FlowAsyncBoundary,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn throttle_definition<In>(capacity: usize, mode: super::ThrottleMode) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let enforcing = matches!(mode, super::ThrottleMode::Enforcing);
  let logic = AsyncBoundaryLogic::<In> { pending: VecDeque::new(), capacity, enforcing };
  FlowDefinition {
    kind:        StageKind::FlowThrottle,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn delay_definition<In>(delay_ticks: u64) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = TimedDelayLogic::<In> {
    mode:       DelayMode::PerElement { delay_ticks },
    pending:    VecDeque::new(),
    tick_count: 0,
  };
  FlowDefinition {
    kind:        StageKind::FlowDelay,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

fn strategy_delay_definition<In, S>(strategy: S) -> FlowDefinition
where
  In: Send + Sync + 'static,
  S: crate::core::delay_strategy::DelayStrategy<In> + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = StrategyDelayLogic::new(strategy);
  FlowDefinition {
    kind:        StageKind::FlowDelay,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn initial_delay_definition<In>(initial_delay_ticks: u64) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = TimedDelayLogic::<In> {
    mode:       DelayMode::Initial { initial_delay_ticks },
    pending:    VecDeque::new(),
    tick_count: 0,
  };
  FlowDefinition {
    kind:        StageKind::FlowInitialDelay,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn take_within_definition<In>(duration_ticks: u64) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = TakeWithinLogic::<In> {
    duration_ticks,
    tick_count: 0,
    expired: false,
    shutdown_requested: false,
    _pd: PhantomData,
  };
  FlowDefinition {
    kind:        StageKind::FlowTakeWithin,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn debounce_definition<In>(silence_ticks: u64) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = DebounceLogic::<In> { silence_ticks, held: None, last_receive_tick: 0, tick_count: 0 };
  FlowDefinition {
    kind:        StageKind::FlowDebounce,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn sample_definition<In>(interval_ticks: u64) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = SampleLogic::<In> { interval_ticks, held: None, last_emit_tick: 0, tick_count: 0 };
  FlowDefinition {
    kind:        StageKind::FlowSample,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn backpressure_timeout_definition<In>(duration_ticks: u64) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = BackpressureTimeoutLogic::<In> {
    duration_ticks,
    tick_count: 0,
    last_apply_tick: 0,
    has_received_element: false,
    _pd: PhantomData,
  };
  FlowDefinition {
    kind:        StageKind::FlowBackpressureTimeout,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn completion_timeout_definition<In>(duration_ticks: u64) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = CompletionTimeoutLogic::<In> { duration_ticks, tick_count: 0, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowCompletionTimeout,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn idle_timeout_definition<In>(duration_ticks: u64) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = IdleTimeoutLogic::<In> { duration_ticks, tick_count: 0, last_element_tick: 0, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowIdleTimeout,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn initial_timeout_definition<In>(duration_ticks: u64) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic =
    InitialTimeoutLogic::<In> { duration_ticks, tick_count: 0, first_element_received: false, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowInitialTimeout,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn batch_definition<In>(size: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = GroupedLogic::<In> { size, current: Vec::new(), source_done: false };
  FlowDefinition {
    kind:        StageKind::FlowBatch,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Vec<In>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn group_by_definition<In, Key, F>(
  max_substreams: usize,
  key_fn: F,
  substream_cancel_strategy: SubstreamCancelStrategy,
) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Key: Clone + PartialEq + Send + Sync + 'static,
  F: FnMut(&In) -> Key + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<(Key, In)> = Outlet::new();
  let logic = GroupByLogic::<In, Key, F> {
    max_substreams,
    seen_keys: Vec::new(),
    key_fn,
    substream_cancel_strategy,
    source_done: false,
    draining: false,
    _pd: PhantomData,
  };
  FlowDefinition {
    kind:        StageKind::FlowGroupBy,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<(Key, In)>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn map_error_definition<In, F>(mapper: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(StreamError) -> StreamError + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = MapErrorLogic { mapper };
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn on_error_continue_definition<In, P, C>(predicate: P, error_consumer: C) -> FlowDefinition
where
  In: Send + Sync + 'static,
  P: FnMut(&StreamError) -> bool + Send + Sync + 'static,
  C: FnMut(&StreamError) + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = OnErrorContinueLogic { predicate, error_consumer };
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn on_error_complete_definition<In, F>(predicate: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(&StreamError) -> bool + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = OnErrorCompleteLogic { predicate };
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn recover_definition<In, F>(recover: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(StreamError) -> Option<In> + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = RecoverLogic::<In, F> { recover, pending: None };
  FlowDefinition {
    kind:        StageKind::FlowRecover,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn recover_with_retries_definition<In, F>(max_retries: isize, recover: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(StreamError) -> Option<Source<In, StreamNotUsed>> + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let retries = if max_retries < 0 { None } else { Some(max_retries as usize) };
  let logic =
    RecoverWithRetriesLogic::<In, F> { max_retries: retries, retries_left: retries, recover, recovery_source: None };
  FlowDefinition {
    kind:        StageKind::FlowRecoverWithRetries,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn split_when_definition<In, F>(predicate: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static, {
  split_when_definition_with_cancel_strategy(predicate, SubstreamCancelStrategy::Propagate)
}

pub(in crate::core) fn split_when_definition_with_cancel_strategy<In, F>(
  predicate: F,
  substream_cancel_strategy: SubstreamCancelStrategy,
) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = SplitWhenLogic::<In, F> {
    predicate,
    substream_cancel_strategy,
    current: Vec::new(),
    source_done: false,
    draining: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowSplitWhen,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Vec<In>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn split_after_definition<In, F>(predicate: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static, {
  split_after_definition_with_cancel_strategy(predicate, SubstreamCancelStrategy::Propagate)
}

pub(in crate::core) fn split_after_definition_with_cancel_strategy<In, F>(
  predicate: F,
  substream_cancel_strategy: SubstreamCancelStrategy,
) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = SplitAfterLogic::<In, F> {
    predicate,
    substream_cancel_strategy,
    current: Vec::new(),
    source_done: false,
    draining: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowSplitAfter,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Vec<In>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn merge_substreams_definition<In>() -> FlowDefinition
where
  In: Send + Sync + 'static, {
  flatten_substreams_definition::<In>(StageKind::FlowMergeSubstreams)
}

pub(in crate::core) fn merge_substreams_with_parallelism_definition<In>(parallelism: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<Vec<In>> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = FlattenSubstreamsWithParallelismLogic::<In> { parallelism, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowMergeSubstreamsWithParallelism,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<Vec<In>>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn concat_substreams_definition<In>() -> FlowDefinition
where
  In: Send + Sync + 'static, {
  flatten_substreams_definition::<In>(StageKind::FlowConcatSubstreams)
}

fn flatten_substreams_definition<In>(kind: StageKind) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<Vec<In>> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = FlattenSubstreamsLogic::<In> { _pd: PhantomData };
  FlowDefinition {
    kind,
    inlet: inlet.id(),
    outlet: outlet.id(),
    input_type: TypeId::of::<Vec<In>>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart: None,
    logic: Box::new(logic),
  }
}

pub(in crate::core) fn broadcast_definition<In>(fan_out: usize) -> FlowDefinition
where
  In: Clone + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = BroadcastLogic::<In> { fan_out, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowBroadcast,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn partition_definition<In, F>(predicate: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = PartitionLogic::<In, F> { predicate, output_slots: VecDeque::new(), _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowPartition,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn balance_definition<In>(fan_out: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = BalanceLogic::<In> { fan_out, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowBalance,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn merge_definition<In>(fan_in: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = MergeLogic::<In> { fan_in, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowMerge,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn merge_preferred_definition<In>(fan_in: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = MergePreferredLogic::<In> {
    fan_in,
    edge_slots: Vec::with_capacity(fan_in),
    pending: Vec::with_capacity(fan_in),
    source_done: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowMergePreferred,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn merge_prioritized_definition<In>(fan_in: usize, priorities: &[usize]) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = MergePrioritizedLogic::<In> {
    fan_in,
    priorities: priorities.to_vec(),
    edge_slots: Vec::with_capacity(fan_in),
    pending: Vec::with_capacity(fan_in),
    credits: Vec::with_capacity(fan_in),
    current: 0,
    source_done: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowMergePrioritized,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn merge_sorted_definition<In>(fan_in: usize) -> FlowDefinition
where
  In: Ord + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = MergeSortedLogic::<In> {
    fan_in,
    edge_slots: Vec::with_capacity(fan_in),
    pending: Vec::with_capacity(fan_in),
    source_done: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowMergeSorted,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn interleave_definition<In>(fan_in: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = InterleaveLogic::<In> {
    fan_in,
    edge_slots: Vec::with_capacity(fan_in),
    pending: Vec::with_capacity(fan_in),
    next_slot: 0,
    source_done: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowInterleave,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn prepend_definition<In>(fan_in: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = ConcatLogic::<In> {
    fan_in,
    edge_slots: Vec::with_capacity(fan_in),
    pending: Vec::with_capacity(fan_in),
    active_slot: 0,
    source_done: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowPrepend,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn prepend_lazy_definition<In, Mat>(source: Source<In, Mat>) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Mat: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = PrependSourceLogic::<In, Mat> {
    secondary:         Some(source),
    secondary_runtime: None,
    pending_secondary: VecDeque::new(),
    pending_primary:   VecDeque::new(),
  };
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn zip_definition<In>(fan_in: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = ZipLogic::<In> { fan_in, edge_slots: Vec::with_capacity(fan_in), pending: Vec::with_capacity(fan_in) };
  FlowDefinition {
    kind:        StageKind::FlowZip,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Vec<In>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn zip_all_definition<In>(fan_in: usize, fill_value: In) -> FlowDefinition
where
  In: Clone + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = ZipAllLogic::<In> {
    fan_in,
    fill_value,
    edge_slots: Vec::with_capacity(fan_in),
    pending: Vec::with_capacity(fan_in),
    source_done: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowZipAll,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Vec<In>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn merge_latest_definition<In>(fan_in: usize) -> FlowDefinition
where
  In: Clone + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = MergeLatestLogic::<In> {
    fan_in,
    edge_slots: Vec::with_capacity(fan_in),
    latest: Vec::with_capacity(fan_in),
    all_seen: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowMergeLatest,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Vec<In>>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn watch_termination_definition<In>(completion: super::StreamCompletion<()>) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = WatchTerminationLogic::<In> { completion, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowWatchTermination,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn kill_switch_definition<In>(kill_switch_state: KillSwitchStateHandle) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = KillSwitchLogic::<In> {
    state:              kill_switch_state,
    shutdown_requested: false,
    _pd:                PhantomData,
  };
  FlowDefinition {
    kind:        StageKind::FlowKillSwitch,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn unzip_definition<In>() -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<(In, In)> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = UnzipLogic::<In> { output_slots: VecDeque::new(), _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowUnzip,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<(In, In)>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn unzip_with_definition<In, Out, F>(func: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> (Out, Out) + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = UnzipWithLogic::<In, Out, F> { func, output_slots: VecDeque::new(), _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowUnzipWith,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn zip_with_index_definition<In>() -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<(In, u64)> = Outlet::new();
  let logic = ZipWithIndexLogic::<In> { next_index: 0, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowZipWithIndex,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<(In, u64)>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn concat_lazy_definition<In, Mat>(source: Source<In, Mat>) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Mat: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = ConcatSourceLogic::<In, Mat> {
    secondary:         Some(source),
    secondary_runtime: None,
    pending:           VecDeque::new(),
    source_done:       false,
  };
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn concat_definition<In>(fan_in: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = ConcatLogic::<In> {
    fan_in,
    edge_slots: Vec::with_capacity(fan_in),
    pending: Vec::with_capacity(fan_in),
    active_slot: 0,
    source_done: false,
  };
  FlowDefinition {
    kind:        StageKind::FlowConcat,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn or_else_definition<In, Mat>(secondary: Source<In, Mat>) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Mat: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = OrElseSourceLogic::<In, Mat> {
    secondary:         Some(secondary),
    secondary_runtime: None,
    pending_secondary: VecDeque::new(),
    emitted_primary:   false,
    source_done:       false,
  };
  FlowDefinition {
    kind:        StageKind::Custom,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

struct MaterializeIntoSourceLogic<Out, F> {
  factory:    Option<F>,
  stream:     Option<Stream>,
  completion: Option<StreamCompletion<Out>>,
  emitted:    bool,
  _pd:        PhantomData<fn() -> Out>,
}

impl<Out, F> MaterializeIntoSourceLogic<Out, F>
where
  Out: Send + Sync + 'static,
  F: FnOnce() -> Result<(Stream, StreamCompletion<Out>), StreamError> + Send + 'static,
{
  fn ensure_stream(&mut self) -> Result<(), StreamError> {
    if self.stream.is_some() {
      return Ok(());
    }

    let Some(factory) = self.factory.take() else {
      return Err(StreamError::MaterializerNotStarted);
    };
    let (stream, completion) = factory()?;
    self.stream = Some(stream);
    self.completion = Some(completion);
    Ok(())
  }

  fn take_materialized_value(&mut self) -> Result<Option<DynValue>, StreamError> {
    let Some(completion) = self.completion.as_ref() else {
      return Ok(None);
    };
    let Some(result) = completion.try_take() else {
      return Ok(None);
    };
    self.emitted = true;
    result.map(|value| Some(Box::new(value) as DynValue))
  }
}

const MATERIALIZE_IDLE_BUDGET: usize = 1024;

impl<Out, F> SourceLogic for MaterializeIntoSourceLogic<Out, F>
where
  Out: Send + Sync + 'static,
  F: FnOnce() -> Result<(Stream, StreamCompletion<Out>), StreamError> + Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    if self.emitted {
      return Ok(None);
    }

    self.ensure_stream()?;

    if let Some(value) = self.take_materialized_value()? {
      return Ok(Some(value));
    }

    // 有界の busy-wait。進捗がこの回数だけ観測できなければ WouldBlock を返し、呼び出し側で再試行する。
    let mut idle_budget = MATERIALIZE_IDLE_BUDGET;
    loop {
      if let Some(value) = self.take_materialized_value()? {
        return Ok(Some(value));
      }

      let Some(stream) = self.stream.as_ref() else {
        return Err(StreamError::MaterializerNotStarted);
      };
      if stream.state().is_terminal() {
        break;
      }

      let Some(stream) = self.stream.as_mut() else {
        return Err(StreamError::MaterializerNotStarted);
      };
      match stream.drive() {
        | DriveOutcome::Progressed => idle_budget = MATERIALIZE_IDLE_BUDGET,
        | DriveOutcome::Idle => {
          if idle_budget == 0 {
            return Err(StreamError::WouldBlock);
          }
          idle_budget = idle_budget.saturating_sub(1);
        },
      }
    }

    self.take_materialized_value()
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    let Some(stream) = self.stream.as_mut() else {
      return Ok(());
    };
    stream.cancel()
  }
}

#[cfg(feature = "compression")]
const MAX_DECOMPRESSED_BYTES: usize = 1024 * 1024;

#[cfg(feature = "compression")]
fn deflate_bytes(bytes: &[u8]) -> Vec<u8> {
  let level = 6_u8;
  miniz_oxide::deflate::compress_to_vec(bytes, level)
}

#[cfg(feature = "compression")]
fn inflate_bytes(bytes: &[u8]) -> Result<Vec<u8>, StreamError> {
  miniz_oxide::inflate::decompress_to_vec_with_limit(bytes, MAX_DECOMPRESSED_BYTES)
    .map_err(|_| StreamError::CompressionError { kind: "deflate" })
}

#[cfg(feature = "compression")]
fn inflate_gzip_payload_bytes(bytes: &[u8]) -> Result<Vec<u8>, StreamError> {
  let limit = MAX_DECOMPRESSED_BYTES.saturating_add(1);
  let decompressed = miniz_oxide::inflate::decompress_to_vec_with_limit(bytes, limit)
    .map_err(|_| StreamError::CompressionError { kind: "deflate" })?;
  if decompressed.len() > MAX_DECOMPRESSED_BYTES {
    return Err(StreamError::CompressionError { kind: "gzip_too_large" });
  }
  Ok(decompressed)
}

#[cfg(feature = "compression")]
fn gzip_bytes(bytes: &[u8]) -> Vec<u8> {
  let payload = deflate_bytes(bytes);
  let mut output = Vec::with_capacity(payload.len() + 18);
  output.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03]);
  output.extend_from_slice(&payload);
  output.extend_from_slice(&crc32(bytes).to_le_bytes());
  // RFC 1952 に従い ISIZE は入力サイズの mod 2^32 を保持する。
  output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
  output
}

#[cfg(feature = "compression")]
fn gunzip_bytes(bytes: &[u8]) -> Result<Vec<u8>, StreamError> {
  if bytes.len() < 18 {
    return Err(StreamError::CompressionError { kind: "gzip_too_short" });
  }
  if bytes[0] != 0x1f || bytes[1] != 0x8b || bytes[2] != 0x08 {
    return Err(StreamError::CompressionError { kind: "gzip_header" });
  }
  let flags = bytes[3];
  if flags & 0b1110_0000 != 0 {
    return Err(StreamError::CompressionError { kind: "gzip_flags" });
  }
  let payload_end = bytes.len().saturating_sub(8);
  let mut payload_start = 10_usize;

  if flags & 0x04 != 0 {
    if payload_start + 2 > payload_end {
      return Err(StreamError::CompressionError { kind: "gzip_extra_len" });
    }
    let extra_len = u16::from_le_bytes([bytes[payload_start], bytes[payload_start + 1]]) as usize;
    payload_start += 2;
    if payload_start + extra_len > payload_end {
      return Err(StreamError::CompressionError { kind: "gzip_extra" });
    }
    payload_start += extra_len;
  }
  if flags & 0x08 != 0 {
    payload_start = consume_gzip_zero_terminated_field(bytes, payload_start, payload_end)?;
  }
  if flags & 0x10 != 0 {
    payload_start = consume_gzip_zero_terminated_field(bytes, payload_start, payload_end)?;
  }
  if flags & 0x02 != 0 {
    if payload_start + 2 > payload_end {
      return Err(StreamError::CompressionError { kind: "gzip_header_crc" });
    }
    payload_start += 2;
  }
  if payload_start > payload_end {
    return Err(StreamError::CompressionError { kind: "gzip_payload_bounds" });
  }

  let payload = &bytes[payload_start..payload_end];
  let expected_crc =
    u32::from_le_bytes([bytes[payload_end], bytes[payload_end + 1], bytes[payload_end + 2], bytes[payload_end + 3]]);
  let expected_len = u32::from_le_bytes([
    bytes[payload_end + 4],
    bytes[payload_end + 5],
    bytes[payload_end + 6],
    bytes[payload_end + 7],
  ]);
  if usize::try_from(expected_len).ok().filter(|expected_len| *expected_len <= MAX_DECOMPRESSED_BYTES).is_none() {
    return Err(StreamError::CompressionError { kind: "gzip_too_large" });
  }
  let decompressed = inflate_gzip_payload_bytes(payload)?;
  if crc32(&decompressed) != expected_crc || (decompressed.len() as u32) != expected_len {
    return Err(StreamError::CompressionError { kind: "gzip_trailer" });
  }
  Ok(decompressed)
}

#[cfg(feature = "compression")]
fn consume_gzip_zero_terminated_field(
  bytes: &[u8],
  mut index: usize,
  payload_end: usize,
) -> Result<usize, StreamError> {
  while index < payload_end {
    if bytes[index] == 0 {
      return Ok(index.saturating_add(1));
    }
    index = index.saturating_add(1);
  }
  Err(StreamError::CompressionError { kind: "gzip_string_field" })
}

#[cfg(feature = "compression")]
fn crc32(bytes: &[u8]) -> u32 {
  let mut crc = 0xffff_ffff_u32;
  for &byte in bytes {
    crc ^= u32::from(byte);
    for _ in 0..8 {
      let mask = (!((crc & 1).wrapping_sub(1))) & 0xedb8_8320;
      crc = (crc >> 1) ^ mask;
    }
  }
  !crc
}

pub(in crate::core::stage::flow) const fn noop_waker() -> Waker {
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

pub(in crate::core) fn combine_mat<Left, Right, C>(left: Left, right: Right) -> C::Out
where
  C: MatCombineRule<Left, Right>, {
  C::combine(left, right)
}

/// Creates a flow definition that retries elements through inner logics with
/// exponential backoff.
pub(in crate::core) fn retry_flow_definition<In, Out, R>(
  inner_logics: Vec<Box<dyn FlowLogic>>,
  decide_retry: R,
  max_retries: usize,
  min_backoff_ticks: u32,
  max_backoff_ticks: u32,
  random_factor_permille: u16,
) -> FlowDefinition
where
  In: Clone + Send + Sync + 'static,
  Out: Send + Sync + 'static,
  R: Fn(&In, &Out) -> Option<In> + Send + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = RetryFlowLogic::<In, Out, R>::new(
    inner_logics,
    decide_retry,
    max_retries,
    min_backoff_ticks,
    max_backoff_ticks,
    random_factor_permille,
  );
  FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}
