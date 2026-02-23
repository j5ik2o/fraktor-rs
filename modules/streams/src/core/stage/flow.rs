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
  DynValue, FlowDefinition, FlowLogic, FlowMonitor, FlowSubFlow, MatCombine, MatCombineRule, RestartBackoff,
  RestartSettings, Source, StageDefinition, StageKind, StreamDslError, StreamError, StreamGraph, StreamNotUsed,
  StreamStage, SupervisionStrategy, downcast_value,
  graph::{GraphStage, GraphStageLogic},
  shape::{Inlet, Outlet, StreamShape},
  sink::Sink,
  stage_context::StageContext,
  validate_positive_argument,
};

#[cfg(test)]
mod tests;

/// Flow stage definition.
pub struct Flow<In, Out, Mat> {
  graph: StreamGraph,
  mat:   Mat,
  _pd:   PhantomData<fn(In) -> Out>,
}

impl<T> Flow<T, T, StreamNotUsed> {
  /// Creates an identity flow.
  #[must_use]
  pub fn new() -> Self {
    Self { graph: StreamGraph::new(), mat: StreamNotUsed::new(), _pd: PhantomData }
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
  pub fn map<T, F>(mut self, func: F) -> Flow<In, T, Mat>
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
    overflow_policy: OverflowPolicy,
  ) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let capacity = validate_positive_argument("capacity", capacity)?;
    let definition = buffer_definition::<Out>(capacity, overflow_policy);
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
  pub fn throttle(mut self, capacity: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    let capacity = validate_positive_argument("capacity", capacity)?;
    let definition = throttle_definition::<Out>(capacity);
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

  /// Adds a group-by stage and returns substream surface for merge operations.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `max_substreams` is zero.
  pub fn group_by<K, F>(
    mut self,
    max_substreams: usize,
    key_fn: F,
  ) -> Result<FlowSubFlow<In, Out, Mat>, StreamDslError>
  where
    K: Clone + PartialEq + Send + Sync + 'static,
    F: FnMut(&Out) -> K + Send + Sync + 'static, {
    let max_substreams = validate_positive_argument("max_substreams", max_substreams)?;
    let definition = group_by_definition::<Out, K, F>(max_substreams, key_fn);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    let grouped = Flow::<In, (K, Out), Mat> { graph: self.graph, mat: self.mat, _pd: PhantomData };
    Ok(FlowSubFlow::from_flow(grouped.map(|(_, value)| vec![value])))
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

  pub(crate) fn from_graph(graph: StreamGraph, mat: Mat) -> Self {
    Self { graph, mat, _pd: PhantomData }
  }

  pub(crate) fn into_parts(self) -> (StreamGraph, Mat) {
    (self.graph, self.mat)
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

impl<In, Out, Mat> Flow<In, Result<Out, StreamError>, Mat>
where
  In: Send + Sync + 'static,
  Out: Clone + Send + Sync + 'static,
{
  /// Maps error payloads while keeping successful elements unchanged.
  #[must_use]
  pub fn map_error<F>(self, mut mapper: F) -> Flow<In, Result<Out, StreamError>, Mat>
  where
    F: FnMut(StreamError) -> StreamError + Send + Sync + 'static, {
    self.map(move |value| value.map_err(&mut mapper))
  }

  /// Drops failing payloads and keeps successful elements.
  #[must_use]
  pub fn on_error_continue(self) -> Flow<In, Out, Mat> {
    self.map_option(Result::ok)
  }

  /// Alias of [`Flow::on_error_continue`].
  #[must_use]
  pub fn on_error_resume(self) -> Flow<In, Out, Mat> {
    self.on_error_continue()
  }

  /// Emits successful payloads until first error payload is observed.
  #[must_use]
  pub fn on_error_complete(self) -> Flow<In, Out, Mat> {
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
  pub fn recover(mut self, fallback: Out) -> Flow<In, Out, Mat> {
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Recovers error payloads while retry budget remains, then fails the stream.
  #[must_use]
  pub fn recover_with_retries(mut self, max_retries: usize, fallback: Out) -> Flow<In, Out, Mat> {
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Alias of [`Flow::recover`].
  #[must_use]
  pub fn recover_with(self, fallback: Out) -> Flow<In, Out, Mat> {
    self.recover(fallback)
  }
}

impl<In, Out, Mat> Flow<In, Out, Mat>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Adds unit context to each output element.
  #[must_use]
  pub fn as_flow_with_context(self) -> Flow<In, ((), Out), Mat> {
    self.map(|value| ((), value))
  }

  /// Keeps only the first element matching `predicate`.
  #[must_use]
  pub fn collect_first<F>(self, predicate: F) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    self.filter(predicate).take(1)
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
    self,
    max_weight: usize,
    _weight_fn: FW,
  ) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
  where
    FW: FnMut(&Out) -> usize + Send + Sync + 'static, {
    self.grouped(max_weight)
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
  pub fn limit_weighted<FW>(self, max_weight: usize, _weight_fn: FW) -> Flow<In, Out, Mat>
  where
    FW: FnMut(&Out) -> usize + Send + Sync + 'static, {
    self.take(max_weight)
  }

  /// Adds a logging stage that passes each element through unchanged.
  ///
  /// In the current no_std configuration this inserts a wire-tap stage
  /// without an output sink. When a logging backend is wired in the
  /// future the tap callback will forward to it.
  #[must_use]
  pub fn log(self, _name: &'static str) -> Flow<In, Out, Mat> {
    self.wire_tap(|_| {})
  }

  /// Adds a marker-tagged logging stage that passes each element through unchanged.
  #[must_use]
  pub fn log_with_marker(self, _name: &'static str, _marker: &'static str) -> Flow<In, Out, Mat> {
    self.wire_tap(|_| {})
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

  /// Adds a materialize-into-source compatibility placeholder.
  #[must_use]
  pub const fn materialize_into_source(self) -> Flow<In, Out, Mat> {
    self
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

  /// Compatibility alias for map-async partitioned entry points.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn map_async_partitioned<T, F, Fut>(
    self,
    parallelism: usize,
    _partitions: usize,
    func: F,
  ) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static, {
    self.map_async(parallelism, func)
  }

  /// Compatibility alias for map-async partitioned unordered entry points.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `parallelism` is zero.
  pub fn map_async_partitioned_unordered<T, F, Fut>(
    self,
    parallelism: usize,
    _partitions: usize,
    func: F,
  ) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    F: FnMut(Out) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = T> + Send + 'static, {
    self.map_async(parallelism, func)
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

  /// Compatibility alias for delay-with entry points.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn delay_with(self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.delay(ticks)
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
    self,
    max_weight: usize,
    _ticks: usize,
    _weight_fn: FW,
  ) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
  where
    FW: FnMut(&Out) -> usize + Send + Sync + 'static, {
    self.grouped(max_weight)
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
    self,
    max_weight: usize,
    _weight_fn: FW,
  ) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
  where
    FW: FnMut(&Out) -> usize + Send + Sync + 'static, {
    self.batch(max_weight)
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

  /// Assigns a debug name to this stage (no-op until Attributes are introduced).
  #[must_use]
  pub const fn named(self, _name: &str) -> Flow<In, Out, Mat> {
    self
  }

  /// Adds a flat-map-prefix compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `prefix` is zero.
  pub fn flat_map_prefix<T, Mat2, F>(self, prefix: usize, mut factory: F) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    T: Send + Sync + 'static,
    Mat2: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> Source<T, Mat2> + Send + Sync + 'static, {
    let _ = validate_positive_argument("prefix", prefix)?;
    self.flat_map_merge(1, move |value| factory(vec![value]))
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

  /// Emits prefix-and-tail compatibility output.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` is zero.
  pub fn prefix_and_tail(self, size: usize) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError> {
    self.grouped(size)
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

  /// Adds a concat-lazy compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn concat_lazy(self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.concat(fan_in)
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

  /// Adds an or-else compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn or_else(self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.prepend(fan_in)
  }

  /// Adds a prepend-lazy compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn prepend_lazy(self, fan_in: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.prepend(fan_in)
  }

  /// Adds a zip-latest compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn zip_latest(self, fan_in: usize, fill_value: Out) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError>
  where
    Out: Clone, {
    self.zip_all(fan_in, fill_value)
  }

  /// Adds a zip-latest-with compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn zip_latest_with<T, F>(
    self,
    fan_in: usize,
    fill_value: Out,
    func: F,
  ) -> Result<Flow<In, T, Mat>, StreamDslError>
  where
    Out: Clone,
    T: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> T + Send + Sync + 'static, {
    Ok(self.zip_latest(fan_in, fill_value)?.map(func))
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

  /// Adds a divert-to compatibility stage.
  #[must_use]
  pub fn divert_to<Mat2, F>(self, predicate: F, sink: Sink<Out, Mat2>) -> Flow<In, Out, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    core::mem::drop(sink);
    self.filter_not(predicate)
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
  #[must_use]
  pub const fn deflate(self) -> Flow<In, Out, Mat> {
    self
  }

  /// Adds a gzip compatibility stage.
  #[must_use]
  pub const fn gzip(self) -> Flow<In, Out, Mat> {
    self
  }

  /// Adds a gzip-decompress compatibility stage.
  #[must_use]
  pub const fn gzip_decompress(self) -> Flow<In, Out, Mat> {
    self
  }

  /// Adds an inflate compatibility stage.
  #[must_use]
  pub const fn inflate(self) -> Flow<In, Out, Mat> {
    self
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
}

impl<In, Out> Flow<In, Out, StreamNotUsed>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
{
  /// Creates a flow from sink-and-source compatibility entry points.
  #[must_use]
  pub fn from_sink_and_source<Mat1, Mat2>(sink: Sink<In, Mat1>, source: Source<Out, Mat2>) -> Self {
    core::mem::drop(sink);
    core::mem::drop(source);
    Self { graph: StreamGraph::new(), mat: StreamNotUsed::new(), _pd: PhantomData }
  }

  /// Creates a coupled flow from sink-and-source compatibility entry points.
  #[must_use]
  pub fn from_sink_and_source_coupled<Mat1, Mat2>(sink: Sink<In, Mat1>, source: Source<Out, Mat2>) -> Self {
    Self::from_sink_and_source(sink, source)
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
  Out: Send + Sync + 'static,
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
  In: Send + Sync + 'static,
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

pub(in crate::core) fn buffer_definition<In>(capacity: usize, overflow_policy: OverflowPolicy) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = BufferLogic::<In> { capacity, overflow_policy, pending: VecDeque::new(), source_done: false };
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
  let logic = AsyncBoundaryLogic::<In> { pending: VecDeque::new(), capacity: 16 };
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

pub(in crate::core) fn throttle_definition<In>(capacity: usize) -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = AsyncBoundaryLogic::<In> { pending: VecDeque::new(), capacity };
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

pub(in crate::core) fn group_by_definition<In, Key, F>(max_substreams: usize, key_fn: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Key: Clone + PartialEq + Send + Sync + 'static,
  F: FnMut(&In) -> Key + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<(Key, In)> = Outlet::new();
  let logic = GroupByLogic::<In, Key, F> { max_substreams, seen_keys: Vec::new(), key_fn, _pd: PhantomData };
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

pub(in crate::core) fn recover_definition<In>(fallback: In) -> FlowDefinition
where
  In: Clone + Send + Sync + 'static, {
  let inlet: Inlet<Result<In, StreamError>> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = RecoverLogic::<In> { fallback };
  FlowDefinition {
    kind:        StageKind::FlowRecover,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<Result<In, StreamError>>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(logic),
  }
}

pub(in crate::core) fn recover_with_retries_definition<In>(max_retries: usize, fallback: In) -> FlowDefinition
where
  In: Clone + Send + Sync + 'static, {
  let inlet: Inlet<Result<In, StreamError>> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = RecoverWithRetriesLogic::<In> { max_retries, retries_left: max_retries, fallback };
  FlowDefinition {
    kind:        StageKind::FlowRecoverWithRetries,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<Result<In, StreamError>>(),
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
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = SplitWhenLogic::<In, F> { predicate, current: Vec::new(), source_done: false };
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
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Vec<In>> = Outlet::new();
  let logic = SplitAfterLogic::<In, F> { predicate, current: Vec::new(), source_done: false };
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

fn watch_termination_definition<In>(completion: super::StreamCompletion<()>) -> FlowDefinition
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

struct LazyFlowLogic<In, Out, Mat, F> {
  factory:      Option<F>,
  inner_logics: Vec<Box<dyn FlowLogic>>,
  // factory 生成 Flow の Mat 値を保持
  mat:          Option<Mat>,
  _pd:          PhantomData<fn(In, Out)>,
}

impl<In, Out, Mat, F> FlowLogic for LazyFlowLogic<In, Out, Mat, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat: Default + Send + 'static,
  F: FnOnce() -> Flow<In, Out, Mat> + Send + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if let Some(factory) = self.factory.take() {
      let flow = factory();
      let (graph, mat) = flow.into_parts();
      self.mat = Some(mat);
      let stages = graph.into_stages();
      for stage in stages {
        if let StageDefinition::Flow(def) = stage {
          self.inner_logics.push(def.logic);
        }
      }
    }

    if self.inner_logics.is_empty() {
      return Ok(vec![input]);
    }

    let mut values = vec![input];
    for logic in &mut self.inner_logics {
      let mut next = Vec::new();
      for v in values {
        next.extend(logic.apply(v)?);
      }
      values = next;
    }
    Ok(values)
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    for logic in &mut self.inner_logics {
      logic.on_tick(tick_count)?;
    }
    Ok(())
  }

  fn can_accept_input(&self) -> bool {
    self.inner_logics.first().is_none_or(|l| l.can_accept_input())
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    for logic in &mut self.inner_logics {
      logic.on_source_done()?;
    }
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let n = self.inner_logics.len();
    let mut result = Vec::new();
    for start in 0..n {
      let mut values = self.inner_logics[start].drain_pending()?;
      for j in (start + 1)..n {
        let mut next = Vec::new();
        for v in values {
          next.extend(self.inner_logics[j].apply(v)?);
        }
        values = next;
      }
      result.extend(values);
    }
    Ok(result)
  }

  fn has_pending_output(&self) -> bool {
    self.inner_logics.iter().any(|l| l.has_pending_output())
  }

  fn take_shutdown_request(&mut self) -> bool {
    // any() の短絡評価を避け、全 inner logic のフラグを一括クリアする
    self.inner_logics.iter_mut().fold(false, |acc, l| l.take_shutdown_request() || acc)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    for logic in &mut self.inner_logics {
      logic.on_restart()?;
    }
    Ok(())
  }
}

struct MapLogic<In, Out, F> {
  func: F,
  _pd:  PhantomData<fn(In) -> Out>,
}

struct MapAsyncLogic<In, Out, F, Fut>
where
  Fut: Future<Output = Out> + Send + 'static, {
  func:        F,
  parallelism: usize,
  pending:     VecDeque<MapAsyncEntry<Out, Fut>>,
  _pd:         PhantomData<fn(In) -> Out>,
}

enum MapAsyncEntry<Out, Fut>
where
  Fut: Future<Output = Out> + Send + 'static, {
  InFlight(Pin<Box<Fut>>),
  Completed(Out),
}

impl<Out, Fut> MapAsyncEntry<Out, Fut>
where
  Fut: Future<Output = Out> + Send + 'static,
{
  const fn is_pending(&self) -> bool {
    match self {
      | Self::InFlight(_) => true,
      | Self::Completed(_) => false,
    }
  }
}

struct StatefulMapLogic<In, Out, Factory, Mapper> {
  factory: Factory,
  mapper:  Mapper,
  _pd:     PhantomData<fn(In) -> Out>,
}

struct StatefulMapConcatLogic<In, Out, Factory, Mapper, I> {
  factory: Factory,
  mapper:  Mapper,
  _pd:     PhantomData<fn(In) -> (Out, I)>,
}

struct MapConcatLogic<In, Out, F, I> {
  func: F,
  _pd:  PhantomData<fn(In) -> (Out, I)>,
}

struct MapOptionLogic<In, Out, F> {
  func: F,
  _pd:  PhantomData<fn(In) -> Out>,
}

struct FilterLogic<In, F> {
  predicate: F,
  _pd:       PhantomData<fn(In)>,
}

struct DropLogic<In> {
  remaining: usize,
  _pd:       PhantomData<fn(In)>,
}

struct TakeLogic<In> {
  remaining: usize,
  _pd:       PhantomData<fn(In)>,
}

struct DropWhileLogic<In, F> {
  predicate: F,
  dropping:  bool,
  _pd:       PhantomData<fn(In)>,
}

struct TakeWhileLogic<In, F> {
  predicate: F,
  taking:    bool,
  _pd:       PhantomData<fn(In)>,
}

struct TakeUntilLogic<In, F> {
  predicate:          F,
  taking:             bool,
  shutdown_requested: bool,
  _pd:                PhantomData<fn(In)>,
}

struct GroupedLogic<In> {
  size:        usize,
  current:     Vec<In>,
  source_done: bool,
}

struct GroupedWithinLogic<In> {
  size:              usize,
  duration_ticks:    u64,
  tick_count:        u64,
  window_start_tick: Option<u64>,
  current:           Vec<In>,
  pending:           VecDeque<Vec<In>>,
}

struct ConflateWithSeedLogic<In, T, FS, FA> {
  seed:         FS,
  aggregate:    FA,
  pending:      Option<T>,
  just_updated: bool,
  _pd:          core::marker::PhantomData<fn(In) -> T>,
}

struct ExpandLogic<In, F> {
  expander:                F,
  last:                    Option<In>,
  pending:                 Option<core::iter::Peekable<Box<dyn Iterator<Item = In> + Send + 'static>>>,
  tick_count:              u64,
  last_input_tick:         Option<u64>,
  last_extrapolation_tick: Option<u64>,
  source_done:             bool,
}

struct SlidingLogic<In> {
  size:   usize,
  window: VecDeque<In>,
}

struct ScanLogic<In, Acc, F> {
  initial:         Acc,
  current:         Acc,
  func:            F,
  initial_emitted: bool,
  source_done:     bool,
  _pd:             PhantomData<fn(In)>,
}

struct IntersperseLogic<In> {
  start:       In,
  inject:      In,
  end:         In,
  pending:     VecDeque<In>,
  needs_start: bool,
  seen_value:  bool,
  source_done: bool,
  end_emitted: bool,
}

impl<In, Out, F> FlowLogic for MapLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Out + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let output = (self.func)(value);
    Ok(vec![Box::new(output)])
  }
}

impl<In, T, FS, FA> FlowLogic for ConflateWithSeedLogic<In, T, FS, FA>
where
  In: Send + Sync + 'static,
  T: Send + Sync + 'static,
  FS: FnMut(In) -> T + Send + Sync + 'static,
  FA: FnMut(T, In) -> T + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let aggregated =
      if let Some(current) = self.pending.take() { (self.aggregate)(current, value) } else { (self.seed)(value) };
    self.pending = Some(aggregated);
    self.just_updated = true;
    Ok(Vec::new())
  }

  fn can_accept_input(&self) -> bool {
    true
  }

  fn can_accept_input_while_output_buffered(&self) -> bool {
    true
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let Some(aggregated) = self.pending.take() else {
      return Ok(Vec::new());
    };

    if self.just_updated {
      self.pending = Some(aggregated);
      self.just_updated = false;
      return Ok(Vec::new());
    }

    Ok(vec![Box::new(aggregated) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    self.pending.is_some()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending = None;
    self.just_updated = false;
    Ok(())
  }
}

impl<In, Out, F, Fut> FlowLogic for MapAsyncLogic<In, Out, F, Fut>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Fut + Send + Sync + 'static,
  Fut: Future<Output = Out> + Send + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let future = (self.func)(value);
    self.pending.push_back(MapAsyncEntry::InFlight(Box::pin(future)));
    Ok(Vec::new())
  }

  fn can_accept_input(&self) -> bool {
    if self.parallelism == 0 {
      return false;
    }
    self.pending.iter().filter(|entry| entry.is_pending()).count() < self.parallelism
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    for entry in &mut self.pending {
      let MapAsyncEntry::InFlight(future) = entry else {
        continue;
      };
      if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
        *entry = MapAsyncEntry::Completed(output);
      }
    }

    let mut outputs = Vec::new();
    while let Some(entry) = self.pending.pop_front() {
      match entry {
        | MapAsyncEntry::Completed(output) => outputs.push(Box::new(output) as DynValue),
        | in_flight => {
          self.pending.push_front(in_flight);
          break;
        },
      }
    }
    Ok(outputs)
  }

  fn has_pending_output(&self) -> bool {
    !self.pending.is_empty()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending.clear();
    Ok(())
  }
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

impl<In, Out, Factory, Mapper> FlowLogic for StatefulMapLogic<In, Out, Factory, Mapper>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Factory: FnMut() -> Mapper + Send + Sync + 'static,
  Mapper: FnMut(In) -> Out + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let output = (self.mapper)(value);
    Ok(vec![Box::new(output) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.mapper = (self.factory)();
    Ok(())
  }
}

impl<In, Out, Factory, Mapper, I> FlowLogic for StatefulMapConcatLogic<In, Out, Factory, Mapper, I>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Factory: FnMut() -> Mapper + Send + Sync + 'static,
  Mapper: FnMut(In) -> I + Send + Sync + 'static,
  I: IntoIterator<Item = Out> + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let output = (self.mapper)(value);
    Ok(output.into_iter().map(|value| Box::new(value) as DynValue).collect())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.mapper = (self.factory)();
    Ok(())
  }
}

impl<In, Out, F, I> FlowLogic for MapConcatLogic<In, Out, F, I>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> I + Send + Sync + 'static,
  I: IntoIterator<Item = Out> + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let output = (self.func)(value);
    Ok(output.into_iter().map(|value| Box::new(value) as DynValue).collect())
  }
}

impl<In, Out, F> FlowLogic for MapOptionLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Option<Out> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let Some(output) = (self.func)(value) else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(output) as DynValue])
  }
}

impl<In, F> FlowLogic for FilterLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if (self.predicate)(&value) {
      return Ok(vec![Box::new(value) as DynValue]);
    }
    Ok(Vec::new())
  }
}

impl<In> FlowLogic for DropLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if self.remaining > 0 {
      self.remaining = self.remaining.saturating_sub(1);
      return Ok(Vec::new());
    }
    Ok(vec![Box::new(value) as DynValue])
  }
}

impl<In> FlowLogic for TakeLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if self.remaining == 0 {
      return Ok(Vec::new());
    }
    self.remaining = self.remaining.saturating_sub(1);
    Ok(vec![Box::new(value) as DynValue])
  }
}

impl<In, F> FlowLogic for DropWhileLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if self.dropping && (self.predicate)(&value) {
      return Ok(Vec::new());
    }
    self.dropping = false;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.dropping = true;
    Ok(())
  }
}

impl<In, F> FlowLogic for TakeWhileLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if !self.taking {
      return Ok(Vec::new());
    }
    if !(self.predicate)(&value) {
      self.taking = false;
      return Ok(Vec::new());
    }
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.taking = true;
    Ok(())
  }
}

impl<In, F> FlowLogic for TakeUntilLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if !self.taking {
      return Ok(Vec::new());
    }
    if (self.predicate)(&value) {
      self.taking = false;
      self.shutdown_requested = true;
      return Ok(vec![Box::new(value) as DynValue]);
    }
    Ok(vec![Box::new(value) as DynValue])
  }

  fn take_shutdown_request(&mut self) -> bool {
    let requested = self.shutdown_requested;
    self.shutdown_requested = false;
    requested
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.taking = true;
    self.shutdown_requested = false;
    Ok(())
  }
}

impl<In> FlowLogic for GroupedLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.size == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    self.current.push(value);
    if self.current.len() < self.size {
      return Ok(Vec::new());
    }
    let output = core::mem::take(&mut self.current);
    Ok(vec![Box::new(output) as DynValue])
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done || self.current.is_empty() {
      return Ok(Vec::new());
    }
    let output = core::mem::take(&mut self.current);
    Ok(vec![Box::new(output) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.current.clear();
    self.source_done = false;
    Ok(())
  }
}

impl<In> GroupedWithinLogic<In>
where
  In: Send + Sync + 'static,
{
  fn tick_window_expired(&self) -> bool {
    self
      .window_start_tick
      .is_some_and(|window_start_tick| self.tick_count >= window_start_tick.saturating_add(self.duration_ticks))
  }

  fn flush_current(&mut self) {
    if self.current.is_empty() {
      return;
    }
    self.pending.push_back(core::mem::take(&mut self.current));
    self.window_start_tick = None;
  }
}

impl<In> FlowLogic for GroupedWithinLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.size == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    if self.current.is_empty() {
      self.window_start_tick = Some(self.tick_count);
    }
    self.current.push(value);
    if self.current.len() >= self.size {
      self.flush_current();
    }
    self.drain_pending()
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    if self.tick_window_expired() {
      self.flush_current();
    }
    Ok(())
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.flush_current();
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let Some(values) = self.pending.pop_front() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(values) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    !self.pending.is_empty()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.tick_count = 0;
    self.window_start_tick = None;
    self.current.clear();
    self.pending.clear();
    Ok(())
  }
}

impl<In, F, I> FlowLogic for ExpandLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> I + Send + Sync + 'static,
  I: IntoIterator<Item = In> + 'static,
  <I as IntoIterator>::IntoIter: Send,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.last = Some(value);
    self.last_input_tick = Some(self.tick_count);
    self.last_extrapolation_tick = Some(self.tick_count);
    let Some(last) = self.last.as_ref() else {
      return Ok(Vec::new());
    };
    let mut iterator = (self.expander)(last).into_iter();
    if self.source_done {
      if let Some(next) = iterator.next() {
        return Ok(vec![Box::new(next) as DynValue]);
      }
      return Ok(Vec::new());
    }
    let iterator: Box<dyn Iterator<Item = In> + Send + 'static> = Box::new(iterator);
    self.pending = Some(iterator.peekable());
    self.drain_pending()
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    Ok(())
  }

  fn can_accept_input(&self) -> bool {
    true
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if self.source_done {
      self.pending = None;
      return Ok(Vec::new());
    }

    if let Some(iter) = &mut self.pending {
      if let Some(value) = iter.next() {
        if iter.peek().is_none() {
          self.pending = None;
        }
        return Ok(vec![Box::new(value) as DynValue]);
      }
      self.pending = None;
    }

    let Some(last_input_tick) = self.last_input_tick else {
      return Ok(Vec::new());
    };
    if self.tick_count <= last_input_tick || self.last_extrapolation_tick == Some(self.tick_count) {
      return Ok(Vec::new());
    }
    let Some(last) = self.last.as_ref() else {
      return Ok(Vec::new());
    };
    self.last_extrapolation_tick = Some(self.tick_count);
    let iterator: Box<dyn Iterator<Item = In> + Send + 'static> = Box::new((self.expander)(last).into_iter());
    self.pending = Some(iterator.peekable());

    if let Some(iter) = &mut self.pending {
      if let Some(value) = iter.next() {
        if iter.peek().is_none() {
          self.pending = None;
        }
        return Ok(vec![Box::new(value) as DynValue]);
      }
      self.pending = None;
    }
    Ok(Vec::new())
  }

  fn has_pending_output(&self) -> bool {
    self.pending.is_some()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.last = None;
    self.pending = None;
    self.tick_count = 0;
    self.last_input_tick = None;
    self.last_extrapolation_tick = None;
    self.source_done = false;
    Ok(())
  }
}

impl<In> FlowLogic for SlidingLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.size == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    self.window.push_back(value);
    if self.window.len() < self.size {
      return Ok(Vec::new());
    }
    if self.window.len() > self.size {
      let _ = self.window.pop_front();
    }
    let output = self.window.iter().cloned().collect::<Vec<In>>();
    let _ = self.window.pop_front();
    Ok(vec![Box::new(output) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.window.clear();
    Ok(())
  }
}

impl<In, Acc, F> FlowLogic for ScanLogic<In, Acc, F>
where
  In: Send + Sync + 'static,
  Acc: Clone + Send + Sync + 'static,
  F: FnMut(Acc, In) -> Acc + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let mut outputs = Vec::new();
    if !self.initial_emitted {
      outputs.push(Box::new(self.current.clone()) as DynValue);
      self.initial_emitted = true;
    }
    let next = (self.func)(self.current.clone(), value);
    self.current = next.clone();
    outputs.push(Box::new(next) as DynValue);
    Ok(outputs)
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done || self.initial_emitted {
      return Ok(Vec::new());
    }
    self.initial_emitted = true;
    Ok(vec![Box::new(self.current.clone()) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.current = self.initial.clone();
    self.initial_emitted = false;
    self.source_done = false;
    Ok(())
  }
}

impl<In> FlowLogic for IntersperseLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if self.needs_start {
      self.pending.push_back(self.start.clone());
      self.needs_start = false;
    }
    if self.seen_value {
      self.pending.push_back(self.inject.clone());
    }
    self.pending.push_back(value);
    self.seen_value = true;
    self.drain_pending()
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if self.source_done {
      if self.needs_start {
        self.pending.push_back(self.start.clone());
        self.needs_start = false;
      }
      if !self.end_emitted {
        self.pending.push_back(self.end.clone());
        self.end_emitted = true;
      }
    }
    Ok(self.pending.drain(..).map(|value| Box::new(value) as DynValue).collect())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending.clear();
    self.needs_start = true;
    self.seen_value = false;
    self.source_done = false;
    self.end_emitted = false;
    Ok(())
  }
}

impl<In, Out, F> GraphStageLogic<In, Out, StreamNotUsed> for MapLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Out + Send + Sync + 'static,
{
  fn on_push(&mut self, ctx: &mut dyn StageContext<In, Out>) {
    let value = ctx.grab();
    let output = (self.func)(value);
    ctx.push(output);
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

struct FlatMapConcatLogic<In, Out, Mat2, F> {
  func:          F,
  active_inner:  Option<VecDeque<Out>>,
  pending_outer: VecDeque<In>,
  _pd:           PhantomData<fn(In) -> (Out, Mat2)>,
}

struct FlatMapMergeLogic<In, Out, Mat2, F> {
  breadth:        usize,
  func:           F,
  active_streams: VecDeque<VecDeque<Out>>,
  pending_outer:  VecDeque<In>,
  _pd:            PhantomData<fn(In) -> (Out, Mat2)>,
}

struct BufferLogic<In> {
  capacity:        usize,
  overflow_policy: OverflowPolicy,
  pending:         VecDeque<In>,
  source_done:     bool,
}

struct AsyncBoundaryLogic<In> {
  pending:  VecDeque<In>,
  capacity: usize,
}

enum DelayMode {
  PerElement { delay_ticks: u64 },
  Initial { initial_delay_ticks: u64 },
}

struct TimedPendingEntry<In> {
  ready_at: u64,
  value:    In,
}

struct TimedDelayLogic<In> {
  mode:       DelayMode,
  pending:    VecDeque<TimedPendingEntry<In>>,
  tick_count: u64,
}

struct TakeWithinLogic<In> {
  duration_ticks:     u64,
  tick_count:         u64,
  expired:            bool,
  shutdown_requested: bool,
  _pd:                PhantomData<fn(In)>,
}

struct BackpressureTimeoutLogic<In> {
  duration_ticks:       u64,
  tick_count:           u64,
  last_apply_tick:      u64,
  has_received_element: bool,
  _pd:                  PhantomData<fn(In)>,
}

struct CompletionTimeoutLogic<In> {
  duration_ticks: u64,
  tick_count:     u64,
  _pd:            PhantomData<fn(In)>,
}

struct IdleTimeoutLogic<In> {
  duration_ticks:    u64,
  tick_count:        u64,
  last_element_tick: u64,
  _pd:               PhantomData<fn(In)>,
}

struct InitialTimeoutLogic<In> {
  duration_ticks:         u64,
  tick_count:             u64,
  first_element_received: bool,
  _pd:                    PhantomData<fn(In)>,
}

struct GroupByLogic<In, Key, F> {
  max_substreams: usize,
  seen_keys:      Vec<Key>,
  key_fn:         F,
  _pd:            PhantomData<fn(In) -> Key>,
}

struct RecoverLogic<In> {
  fallback: In,
}

struct RecoverWithRetriesLogic<In> {
  max_retries:  usize,
  retries_left: usize,
  fallback:     In,
}

struct SplitWhenLogic<In, F> {
  predicate:   F,
  current:     Vec<In>,
  source_done: bool,
}

struct SplitAfterLogic<In, F> {
  predicate:   F,
  current:     Vec<In>,
  source_done: bool,
}

struct FlattenSubstreamsLogic<In> {
  _pd: PhantomData<fn(In)>,
}

struct FlattenSubstreamsWithParallelismLogic<In> {
  parallelism: usize,
  _pd:         PhantomData<fn(In)>,
}

impl<In, Out, Mat2, F> FlatMapConcatLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn promote_outer_if_needed(&mut self) -> Result<(), StreamError> {
    while self.active_inner.is_none() {
      let Some(outer) = self.pending_outer.pop_front() else {
        return Ok(());
      };
      let source = (self.func)(outer);
      let outputs = source.collect_values()?;
      if outputs.is_empty() {
        continue;
      }
      let mut stream = VecDeque::with_capacity(outputs.len());
      stream.extend(outputs);
      self.active_inner = Some(stream);
    }
    Ok(())
  }

  fn pop_next_value(&mut self) -> Result<Option<Out>, StreamError> {
    self.promote_outer_if_needed()?;
    let Some(stream) = &mut self.active_inner else {
      return Ok(None);
    };
    let value = stream.pop_front();
    if stream.is_empty() {
      self.active_inner = None;
    }
    Ok(value)
  }
}

impl<In, Out, Mat2, F> FlowLogic for FlatMapConcatLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.pending_outer.push_back(value);
    self.drain_pending()
  }

  fn can_accept_input(&self) -> bool {
    self.active_inner.is_none() && self.pending_outer.is_empty()
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if let Some(output) = self.pop_next_value()? {
      return Ok(vec![Box::new(output) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.active_inner = None;
    self.pending_outer.clear();
    Ok(())
  }
}

impl<In, Out, Mat2, F> FlatMapMergeLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn enqueue_active_inner(&mut self, value: In) -> Result<(), StreamError> {
    let source = (self.func)(value);
    let outputs = source.collect_values()?;
    if outputs.is_empty() {
      return Ok(());
    }
    let mut stream = VecDeque::with_capacity(outputs.len());
    stream.extend(outputs);
    self.active_streams.push_back(stream);
    Ok(())
  }

  fn promote_pending(&mut self) -> Result<(), StreamError> {
    while self.active_streams.len() < self.breadth {
      let Some(value) = self.pending_outer.pop_front() else {
        break;
      };
      self.enqueue_active_inner(value)?;
    }
    Ok(())
  }

  fn pop_next_value(&mut self) -> Result<Option<Out>, StreamError> {
    self.promote_pending()?;
    loop {
      let Some(mut stream) = self.active_streams.pop_front() else {
        return Ok(None);
      };
      let Some(value) = stream.pop_front() else {
        self.promote_pending()?;
        continue;
      };
      if stream.is_empty() {
        self.promote_pending()?;
      } else {
        self.active_streams.push_back(stream);
      }
      return Ok(Some(value));
    }
  }
}

impl<In, Out, Mat2, F> FlowLogic for FlatMapMergeLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.breadth == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    self.pending_outer.push_back(value);
    if let Some(output) = self.pop_next_value()? {
      return Ok(vec![Box::new(output) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn can_accept_input(&self) -> bool {
    self.breadth > 0 && self.pending_outer.is_empty() && self.active_streams.len() < self.breadth
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if let Some(output) = self.pop_next_value()? {
      return Ok(vec![Box::new(output) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.active_streams.clear();
    self.pending_outer.clear();
    Ok(())
  }
}

impl<In, Out, Mat2, F> GraphStageLogic<In, Out, StreamNotUsed> for FlatMapConcatLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn on_push(&mut self, ctx: &mut dyn StageContext<In, Out>) {
    let value = ctx.grab();
    self.pending_outer.push_back(value);
    match self.pop_next_value() {
      | Ok(Some(output)) => ctx.push(output),
      | Ok(None) => {},
      | Err(error) => ctx.fail(error),
    }
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

impl<In, Out, Mat2, F> GraphStageLogic<In, Out, StreamNotUsed> for FlatMapMergeLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn on_push(&mut self, ctx: &mut dyn StageContext<In, Out>) {
    let value = ctx.grab();
    self.pending_outer.push_back(value);
    match self.pop_next_value() {
      | Ok(Some(output)) => ctx.push(output),
      | Ok(None) => {},
      | Err(error) => ctx.fail(error),
    }
  }

  fn materialized(&mut self) -> StreamNotUsed {
    StreamNotUsed::new()
  }
}

impl<In> BufferLogic<In>
where
  In: Send + Sync + 'static,
{
  fn offer_with_strategy(&mut self, value: In) -> Result<(), StreamError> {
    if self.capacity == 0 {
      return Err(StreamError::InvalidConnection);
    }
    if self.pending.len() < self.capacity {
      self.pending.push_back(value);
      return Ok(());
    }

    match self.overflow_policy {
      | OverflowPolicy::Block => Err(StreamError::BufferOverflow),
      | OverflowPolicy::DropNewest => Ok(()),
      | OverflowPolicy::DropOldest => {
        let _ = self.pending.pop_front();
        self.pending.push_back(value);
        Ok(())
      },
      | OverflowPolicy::Grow => {
        self.pending.push_back(value);
        Ok(())
      },
    }
  }
}

impl<In> FlowLogic for BufferLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.offer_with_strategy(value)?;
    Ok(Vec::new())
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done {
      return Ok(Vec::new());
    }
    let Some(value) = self.pending.pop_front() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(value) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    !self.pending.is_empty()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending.clear();
    self.source_done = false;
    Ok(())
  }
}

impl<In> FlowLogic for AsyncBoundaryLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.pending.push_back(value);
    Ok(Vec::new())
  }

  fn can_accept_input(&self) -> bool {
    self.pending.len() < self.capacity
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let Some(value) = self.pending.pop_front() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending.clear();
    Ok(())
  }
}

impl<In> TimedDelayLogic<In>
where
  In: Send + Sync + 'static,
{
  const fn ready_at(&self) -> u64 {
    match self.mode {
      | DelayMode::PerElement { delay_ticks } => self.tick_count.saturating_add(delay_ticks),
      | DelayMode::Initial { initial_delay_ticks } => {
        if self.tick_count < initial_delay_ticks {
          initial_delay_ticks
        } else {
          self.tick_count
        }
      },
    }
  }
}

impl<In> FlowLogic for TimedDelayLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let ready_at = self.ready_at();
    self.pending.push_back(TimedPendingEntry { ready_at, value });
    Ok(Vec::new())
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let Some(entry) = self.pending.front() else {
      return Ok(Vec::new());
    };
    if entry.ready_at > self.tick_count {
      return Ok(Vec::new());
    }
    let Some(entry) = self.pending.pop_front() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(entry.value) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    !self.pending.is_empty()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending.clear();
    self.tick_count = 0;
    Ok(())
  }
}

impl<In> FlowLogic for TakeWithinLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if self.expired {
      return Ok(Vec::new());
    }
    if self.tick_count > self.duration_ticks {
      self.expired = true;
      self.shutdown_requested = true;
      return Ok(Vec::new());
    }
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    if self.tick_count > self.duration_ticks {
      self.expired = true;
      self.shutdown_requested = true;
    }
    Ok(())
  }

  fn take_shutdown_request(&mut self) -> bool {
    let requested = self.shutdown_requested;
    self.shutdown_requested = false;
    requested
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.tick_count = 0;
    self.expired = false;
    self.shutdown_requested = false;
    Ok(())
  }
}

impl<In> FlowLogic for BackpressureTimeoutLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.has_received_element = true;
    self.last_apply_tick = self.tick_count;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    if self.has_received_element && self.tick_count.saturating_sub(self.last_apply_tick) > self.duration_ticks {
      return Err(StreamError::Timeout { kind: "backpressure", ticks: self.duration_ticks });
    }
    Ok(())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.tick_count = 0;
    self.last_apply_tick = 0;
    self.has_received_element = false;
    Ok(())
  }
}

impl<In> FlowLogic for CompletionTimeoutLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    if self.tick_count > self.duration_ticks {
      return Err(StreamError::Timeout { kind: "completion", ticks: self.duration_ticks });
    }
    Ok(())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.tick_count = 0;
    Ok(())
  }
}

impl<In> FlowLogic for IdleTimeoutLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.last_element_tick = self.tick_count;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    if self.tick_count.saturating_sub(self.last_element_tick) > self.duration_ticks {
      return Err(StreamError::Timeout { kind: "idle", ticks: self.duration_ticks });
    }
    Ok(())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.tick_count = 0;
    self.last_element_tick = 0;
    Ok(())
  }
}

impl<In> FlowLogic for InitialTimeoutLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.first_element_received = true;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    if !self.first_element_received && self.tick_count > self.duration_ticks {
      return Err(StreamError::Timeout { kind: "initial", ticks: self.duration_ticks });
    }
    Ok(())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.tick_count = 0;
    self.first_element_received = false;
    Ok(())
  }
}

impl<In, Key, F> FlowLogic for GroupByLogic<In, Key, F>
where
  In: Send + Sync + 'static,
  Key: Clone + PartialEq + Send + Sync + 'static,
  F: FnMut(&In) -> Key + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.max_substreams == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let key = (self.key_fn)(&value);
    if !self.seen_keys.contains(&key) {
      if self.seen_keys.len() >= self.max_substreams {
        return Err(StreamError::SubstreamLimitExceeded { max_substreams: self.max_substreams });
      }
      self.seen_keys.push(key.clone());
    }
    Ok(vec![Box::new((key, value)) as DynValue])
  }
}

impl<In> FlowLogic for RecoverLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<Result<In, StreamError>>(input)?;
    match value {
      | Ok(value) => Ok(vec![Box::new(value) as DynValue]),
      | Err(_) => Ok(vec![Box::new(self.fallback.clone()) as DynValue]),
    }
  }
}

impl<In> FlowLogic for RecoverWithRetriesLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<Result<In, StreamError>>(input)?;
    match value {
      | Ok(value) => Ok(vec![Box::new(value) as DynValue]),
      | Err(_) => {
        if self.retries_left == 0 {
          return Err(StreamError::Failed);
        }
        self.retries_left = self.retries_left.saturating_sub(1);
        Ok(vec![Box::new(self.fallback.clone()) as DynValue])
      },
    }
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.retries_left = self.max_retries;
    Ok(())
  }
}

impl<In, F> FlowLogic for SplitWhenLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let should_split = (self.predicate)(&value);
    if should_split && !self.current.is_empty() {
      let output = core::mem::take(&mut self.current);
      self.current.push(value);
      return Ok(vec![Box::new(output) as DynValue]);
    }
    self.current.push(value);
    Ok(Vec::new())
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done || self.current.is_empty() {
      return Ok(Vec::new());
    }
    let output = core::mem::take(&mut self.current);
    Ok(vec![Box::new(output) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.current.clear();
    self.source_done = false;
    Ok(())
  }
}

impl<In, F> FlowLogic for SplitAfterLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let should_split = (self.predicate)(&value);
    self.current.push(value);
    if !should_split {
      return Ok(Vec::new());
    }
    let output = core::mem::take(&mut self.current);
    Ok(vec![Box::new(output) as DynValue])
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done || self.current.is_empty() {
      return Ok(Vec::new());
    }
    let output = core::mem::take(&mut self.current);
    Ok(vec![Box::new(output) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.current.clear();
    self.source_done = false;
    Ok(())
  }
}

impl<In> FlowLogic for FlattenSubstreamsLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let values = downcast_value::<Vec<In>>(input)?;
    Ok(values.into_iter().map(|value| Box::new(value) as DynValue).collect())
  }
}

impl<In> FlowLogic for FlattenSubstreamsWithParallelismLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.parallelism == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let values = downcast_value::<Vec<In>>(input)?;
    Ok(values.into_iter().map(|value| Box::new(value) as DynValue).collect())
  }
}

struct BroadcastLogic<In> {
  fan_out: usize,
  _pd:     PhantomData<fn(In)>,
}

impl<In> FlowLogic for BroadcastLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_out == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let mut outputs = Vec::with_capacity(self.fan_out);
    for _ in 0..self.fan_out {
      outputs.push(Box::new(value.clone()) as DynValue);
    }
    Ok(outputs)
  }

  fn expected_fan_out(&self) -> Option<usize> {
    Some(self.fan_out)
  }
}

struct PartitionLogic<In, F> {
  predicate:    F,
  output_slots: VecDeque<usize>,
  _pd:          PhantomData<fn(In)>,
}

impl<In, F> FlowLogic for PartitionLogic<In, F>
where
  In: Send + Sync + 'static,
  F: FnMut(&In) -> bool + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let slot = if (self.predicate)(&value) { 0 } else { 1 };
    self.output_slots.push_back(slot);
    Ok(vec![Box::new(value) as DynValue])
  }

  fn take_next_output_edge_slot(&mut self) -> Option<usize> {
    self.output_slots.pop_front()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.output_slots.clear();
    Ok(())
  }
}

struct BalanceLogic<In> {
  fan_out: usize,
  _pd:     PhantomData<fn(In)>,
}

impl<In> FlowLogic for BalanceLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_out == 0 {
      return Err(StreamError::InvalidConnection);
    }
    Ok(vec![input])
  }

  fn expected_fan_out(&self) -> Option<usize> {
    Some(self.fan_out)
  }
}

struct MergeLogic<In> {
  fan_in: usize,
  _pd:    PhantomData<fn(In)>,
}

impl<In> FlowLogic for MergeLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_in == 0 {
      return Err(StreamError::InvalidConnection);
    }
    Ok(vec![input])
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }
}

struct MergePreferredLogic<In> {
  fan_in:      usize,
  edge_slots:  Vec<usize>,
  pending:     Vec<VecDeque<In>>,
  source_done: bool,
}

impl<In> MergePreferredLogic<In>
where
  In: Send + Sync + 'static,
{
  fn slot_for_edge(&mut self, edge_index: usize) -> Result<usize, StreamError> {
    if let Some(position) = self.edge_slots.iter().position(|index| *index == edge_index) {
      return Ok(position);
    }
    if self.edge_slots.len() >= self.fan_in {
      return Err(StreamError::InvalidConnection);
    }
    let insert_at = self.edge_slots.partition_point(|index| *index < edge_index);
    self.edge_slots.insert(insert_at, edge_index);
    self.pending.insert(insert_at, VecDeque::new());
    Ok(insert_at)
  }

  fn pop_preferred(&mut self) -> Option<In> {
    if self.pending.is_empty() {
      return None;
    }
    // slot 0 はpreferred入力。常に最初にチェックする。
    if let Some(value) = self.pending[0].pop_front() {
      return Some(value);
    }
    // preferred が空の場合のみ他のスロットから取得
    for slot in 1..self.pending.len() {
      if let Some(value) = self.pending[slot].pop_front() {
        return Some(value);
      }
    }
    None
  }
}

impl<In> FlowLogic for MergePreferredLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    self.apply_with_edge(0, input)
  }

  fn apply_with_edge(&mut self, edge_index: usize, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_in == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let slot = self.slot_for_edge(edge_index)?;
    self.pending[slot].push_back(value);
    if let Some(next) = self.pop_preferred() {
      return Ok(vec![Box::new(next) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done {
      return Ok(Vec::new());
    }
    let Some(next) = self.pop_preferred() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(next) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    self.source_done = false;
    Ok(())
  }
}

struct MergePrioritizedLogic<In> {
  fan_in:      usize,
  priorities:  Vec<usize>,
  edge_slots:  Vec<usize>,
  pending:     Vec<VecDeque<In>>,
  credits:     Vec<usize>,
  current:     usize,
  source_done: bool,
}

impl<In> MergePrioritizedLogic<In>
where
  In: Send + Sync + 'static,
{
  fn slot_for_edge(&mut self, edge_index: usize) -> Result<usize, StreamError> {
    if let Some(position) = self.edge_slots.iter().position(|index| *index == edge_index) {
      return Ok(position);
    }
    if self.edge_slots.len() >= self.fan_in {
      return Err(StreamError::InvalidConnection);
    }
    let insert_at = self.edge_slots.partition_point(|index| *index < edge_index);
    self.edge_slots.insert(insert_at, edge_index);
    self.pending.insert(insert_at, VecDeque::new());
    // 仮クレジットを挿入後、全スロットのクレジットを再計算する。
    // 挿入によりスロットがシフトするため、個別設定だとpriorities[slot]との不整合が発生する。
    self.credits.insert(insert_at, 0);
    self.refill_credits();
    if insert_at <= self.current && self.edge_slots.len() > 1 {
      self.current = self.current.saturating_add(1) % self.edge_slots.len();
    }
    Ok(insert_at)
  }

  fn refill_credits(&mut self) {
    for (slot, credit) in self.credits.iter_mut().enumerate() {
      *credit = self.priorities[slot];
    }
  }

  fn pop_prioritized(&mut self) -> Option<In> {
    if self.pending.is_empty() {
      return None;
    }
    let len = self.pending.len();
    // 加重ラウンドロビン: 現在のスロットからクレジットに基づいて要素を取得
    for _ in 0..len {
      let slot = self.current % len;
      if self.credits[slot] > 0
        && let Some(value) = self.pending[slot].pop_front()
      {
        self.credits[slot] = self.credits[slot].saturating_sub(1);
        if self.credits[slot] == 0 {
          self.current = (slot + 1) % len;
        }
        return Some(value);
      }
      self.current = (slot + 1) % len;
    }
    // 全クレジット消費済み → 再充填して再試行
    self.refill_credits();
    for _ in 0..len {
      let slot = self.current % len;
      if let Some(value) = self.pending[slot].pop_front() {
        self.credits[slot] = self.credits[slot].saturating_sub(1);
        if self.credits[slot] == 0 {
          self.current = (slot + 1) % len;
        }
        return Some(value);
      }
      self.current = (slot + 1) % len;
    }
    None
  }
}

impl<In> FlowLogic for MergePrioritizedLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    self.apply_with_edge(0, input)
  }

  fn apply_with_edge(&mut self, edge_index: usize, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_in == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let slot = self.slot_for_edge(edge_index)?;
    self.pending[slot].push_back(value);
    if let Some(next) = self.pop_prioritized() {
      return Ok(vec![Box::new(next) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done {
      return Ok(Vec::new());
    }
    let Some(next) = self.pop_prioritized() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(next) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    self.credits.clear();
    self.current = 0;
    self.source_done = false;
    Ok(())
  }
}

struct MergeSortedLogic<In> {
  fan_in:      usize,
  edge_slots:  Vec<usize>,
  pending:     Vec<VecDeque<In>>,
  source_done: bool,
}

impl<In> MergeSortedLogic<In>
where
  In: Ord + Send + Sync + 'static,
{
  fn slot_for_edge(&mut self, edge_index: usize) -> Result<usize, StreamError> {
    if let Some(position) = self.edge_slots.iter().position(|index| *index == edge_index) {
      return Ok(position);
    }
    if self.edge_slots.len() >= self.fan_in {
      return Err(StreamError::InvalidConnection);
    }
    let insert_at = self.edge_slots.partition_point(|index| *index < edge_index);
    self.edge_slots.insert(insert_at, edge_index);
    self.pending.insert(insert_at, VecDeque::new());
    Ok(insert_at)
  }

  fn pop_sorted(&mut self) -> Option<In> {
    if self.pending.is_empty() {
      return None;
    }
    // source_done前は全fan_inスロットが登録され、かつ全てに要素が揃うのを待つ
    if !self.source_done {
      if self.pending.len() < self.fan_in {
        return None;
      }
      let all_have_data = self.pending.iter().all(|queue| !queue.is_empty());
      if !all_have_data {
        return None;
      }
    }
    // 全スロットの先頭要素を比較して最小値のスロットを選択
    let mut min_slot: Option<usize> = None;
    for (slot, queue) in self.pending.iter().enumerate() {
      if let Some(front) = queue.front() {
        match min_slot {
          | None => min_slot = Some(slot),
          | Some(current_min) => {
            if let Some(current_front) = self.pending[current_min].front()
              && front < current_front
            {
              min_slot = Some(slot);
            }
          },
        }
      }
    }
    min_slot.and_then(|slot| self.pending[slot].pop_front())
  }
}

impl<In> FlowLogic for MergeSortedLogic<In>
where
  In: Ord + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    self.apply_with_edge(0, input)
  }

  fn apply_with_edge(&mut self, edge_index: usize, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_in == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let slot = self.slot_for_edge(edge_index)?;
    self.pending[slot].push_back(value);
    if let Some(next) = self.pop_sorted() {
      return Ok(vec![Box::new(next) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done {
      return Ok(Vec::new());
    }
    let Some(next) = self.pop_sorted() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(next) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    self.source_done = false;
    Ok(())
  }
}

struct InterleaveLogic<In> {
  fan_in:      usize,
  edge_slots:  Vec<usize>,
  pending:     Vec<VecDeque<In>>,
  next_slot:   usize,
  source_done: bool,
}

struct ZipLogic<In> {
  fan_in:     usize,
  edge_slots: Vec<usize>,
  pending:    Vec<VecDeque<In>>,
}

struct ZipAllLogic<In> {
  fan_in:      usize,
  fill_value:  In,
  edge_slots:  Vec<usize>,
  pending:     Vec<VecDeque<In>>,
  source_done: bool,
}

struct MergeLatestLogic<In> {
  fan_in:     usize,
  edge_slots: Vec<usize>,
  latest:     Vec<Option<In>>,
  all_seen:   bool,
}

struct WatchTerminationLogic<In> {
  completion: super::StreamCompletion<()>,
  _pd:        PhantomData<fn(In)>,
}

struct UnzipLogic<In> {
  output_slots: VecDeque<usize>,
  _pd:          PhantomData<fn(In)>,
}

struct UnzipWithLogic<In, Out, F> {
  func:         F,
  output_slots: VecDeque<usize>,
  _pd:          PhantomData<fn(In) -> Out>,
}

struct ZipWithIndexLogic<In> {
  next_index: u64,
  _pd:        PhantomData<fn(In)>,
}

impl<In> InterleaveLogic<In>
where
  In: Send + Sync + 'static,
{
  fn slot_for_edge(&mut self, edge_index: usize) -> Result<usize, StreamError> {
    if let Some(position) = self.edge_slots.iter().position(|index| *index == edge_index) {
      return Ok(position);
    }
    if self.edge_slots.len() >= self.fan_in {
      return Err(StreamError::InvalidConnection);
    }
    let insert_at = self.edge_slots.partition_point(|index| *index < edge_index);
    self.edge_slots.insert(insert_at, edge_index);
    self.pending.insert(insert_at, VecDeque::new());
    if insert_at <= self.next_slot && self.edge_slots.len() > 1 {
      self.next_slot = self.next_slot.saturating_add(1) % self.edge_slots.len();
    }
    Ok(insert_at)
  }

  fn pop_next_value(&mut self) -> Option<In> {
    if self.pending.is_empty() {
      return None;
    }
    let start_slot = self.next_slot % self.pending.len();
    let mut slot = start_slot;
    for _ in 0..self.pending.len() {
      if let Some(value) = self.pending[slot].pop_front() {
        self.next_slot = (slot + 1) % self.pending.len();
        return Some(value);
      }
      slot = (slot + 1) % self.pending.len();
    }
    None
  }
}

impl<In> FlowLogic for InterleaveLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    self.apply_with_edge(0, input)
  }

  fn apply_with_edge(&mut self, edge_index: usize, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_in == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let slot = self.slot_for_edge(edge_index)?;
    self.pending[slot].push_back(value);
    if let Some(next) = self.pop_next_value() {
      return Ok(vec![Box::new(next) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done {
      return Ok(Vec::new());
    }
    let Some(next) = self.pop_next_value() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(next) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    self.next_slot = 0;
    self.source_done = false;
    Ok(())
  }
}

impl<In> ZipLogic<In>
where
  In: Send + Sync + 'static,
{
  fn slot_for_edge(&mut self, edge_index: usize) -> Result<usize, StreamError> {
    if let Some(position) = self.edge_slots.iter().position(|index| *index == edge_index) {
      return Ok(position);
    }
    if self.edge_slots.len() >= self.fan_in {
      return Err(StreamError::InvalidConnection);
    }
    self.edge_slots.push(edge_index);
    self.pending.push(VecDeque::new());
    Ok(self.edge_slots.len().saturating_sub(1))
  }
}

impl<In> ZipAllLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn slot_for_edge(&mut self, edge_index: usize) -> Result<usize, StreamError> {
    if let Some(position) = self.edge_slots.iter().position(|index| *index == edge_index) {
      return Ok(position);
    }
    if self.edge_slots.len() >= self.fan_in {
      return Err(StreamError::InvalidConnection);
    }
    let insert_at = self.edge_slots.partition_point(|index| *index < edge_index);
    self.edge_slots.insert(insert_at, edge_index);
    self.pending.insert(insert_at, VecDeque::new());
    Ok(insert_at)
  }

  fn pop_ready_group(&mut self) -> Option<Vec<In>> {
    if self.pending.len() < self.fan_in {
      return None;
    }
    let ready = self.pending.iter().all(|queue| !queue.is_empty());
    if !ready {
      return None;
    }
    let mut values = Vec::with_capacity(self.fan_in);
    for queue in &mut self.pending {
      let value = queue.pop_front()?;
      values.push(value);
    }
    Some(values)
  }

  fn pop_with_fill_after_completion(&mut self) -> Option<Vec<In>> {
    if self.pending.iter().all(|queue| queue.is_empty()) {
      return None;
    }
    let mut values = Vec::with_capacity(self.fan_in);
    for queue in &mut self.pending {
      if let Some(value) = queue.pop_front() {
        values.push(value);
      } else {
        values.push(self.fill_value.clone());
      }
    }
    for _ in self.pending.len()..self.fan_in {
      values.push(self.fill_value.clone());
    }
    Some(values)
  }
}

impl<In> FlowLogic for ZipAllLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    self.apply_with_edge(0, input)
  }

  fn apply_with_edge(&mut self, edge_index: usize, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_in == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let slot = self.slot_for_edge(edge_index)?;
    self.pending[slot].push_back(value);

    if let Some(values) = self.pop_ready_group() {
      return Ok(vec![Box::new(values) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if let Some(values) = self.pop_ready_group() {
      return Ok(vec![Box::new(values) as DynValue]);
    }
    if !self.source_done {
      return Ok(Vec::new());
    }
    let Some(values) = self.pop_with_fill_after_completion() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(values) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    self.source_done = false;
    Ok(())
  }
}

// --- MergeLatestLogic ---

impl<In> MergeLatestLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn slot_for_edge(&mut self, edge_index: usize) -> Result<usize, StreamError> {
    if let Some(position) = self.edge_slots.iter().position(|index| *index == edge_index) {
      return Ok(position);
    }
    if self.edge_slots.len() >= self.fan_in {
      return Err(StreamError::InvalidConnection);
    }
    let insert_at = self.edge_slots.partition_point(|index| *index < edge_index);
    self.edge_slots.insert(insert_at, edge_index);
    self.latest.insert(insert_at, None);
    Ok(insert_at)
  }

  fn try_emit(&self) -> Option<Vec<In>> {
    if !self.all_seen {
      return None;
    }
    Some(self.latest.iter().filter_map(|opt| opt.as_ref().cloned()).collect())
  }
}

impl<In> FlowLogic for MergeLatestLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    self.apply_with_edge(0, input)
  }

  fn apply_with_edge(&mut self, edge_index: usize, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_in == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let slot = self.slot_for_edge(edge_index)?;
    self.latest[slot] = Some(value);
    // 全スロットが一度でもSomeになったかチェック
    if !self.all_seen && self.latest.len() >= self.fan_in && self.latest.iter().all(|opt| opt.is_some()) {
      self.all_seen = true;
    }
    if let Some(values) = self.try_emit() {
      return Ok(vec![Box::new(values) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.latest.clear();
    self.all_seen = false;
    Ok(())
  }
}

// --- WatchTerminationLogic ---

impl<In> FlowLogic for WatchTerminationLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    Ok(vec![input])
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.completion.complete(Ok(()));
    Ok(())
  }
}

impl<In> FlowLogic for UnzipLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let (left, right) = downcast_value::<(In, In)>(input)?;
    self.output_slots.push_back(0);
    self.output_slots.push_back(1);
    Ok(vec![Box::new(left) as DynValue, Box::new(right) as DynValue])
  }

  fn take_next_output_edge_slot(&mut self) -> Option<usize> {
    self.output_slots.pop_front()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.output_slots.clear();
    Ok(())
  }
}

impl<In, Out, F> FlowLogic for UnzipWithLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> (Out, Out) + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let (left, right) = (self.func)(value);
    self.output_slots.push_back(0);
    self.output_slots.push_back(1);
    Ok(vec![Box::new(left) as DynValue, Box::new(right) as DynValue])
  }

  fn take_next_output_edge_slot(&mut self) -> Option<usize> {
    self.output_slots.pop_front()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.output_slots.clear();
    Ok(())
  }
}

impl<In> FlowLogic for ZipLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    self.apply_with_edge(0, input)
  }

  fn apply_with_edge(&mut self, edge_index: usize, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_in == 0 {
      return Err(StreamError::InvalidConnection);
    }

    let value = downcast_value::<In>(input)?;
    let slot = self.slot_for_edge(edge_index)?;
    self.pending[slot].push_back(value);

    if self.edge_slots.len() < self.fan_in {
      return Ok(Vec::new());
    }

    let ready = self.pending.iter().all(|queue| !queue.is_empty());
    if !ready {
      return Ok(Vec::new());
    }

    let mut zipped = Vec::with_capacity(self.fan_in);
    for queue in &mut self.pending {
      let Some(item) = queue.pop_front() else {
        return Err(StreamError::InvalidConnection);
      };
      zipped.push(item);
    }

    Ok(vec![Box::new(zipped) as DynValue])
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    Ok(())
  }
}

impl<In> FlowLogic for ZipWithIndexLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let index = self.next_index;
    self.next_index = self.next_index.saturating_add(1);
    Ok(vec![Box::new((value, index)) as DynValue])
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.next_index = 0;
    Ok(())
  }
}

struct ConcatLogic<In> {
  fan_in:      usize,
  edge_slots:  Vec<usize>,
  pending:     Vec<VecDeque<In>>,
  active_slot: usize,
  source_done: bool,
}

impl<In> ConcatLogic<In>
where
  In: Send + Sync + 'static,
{
  fn slot_for_edge(&mut self, edge_index: usize) -> Result<usize, StreamError> {
    if let Some(position) = self.edge_slots.iter().position(|index| *index == edge_index) {
      return Ok(position);
    }
    if self.edge_slots.len() >= self.fan_in {
      return Err(StreamError::InvalidConnection);
    }
    let insert_at = self.edge_slots.partition_point(|index| *index < edge_index);
    self.edge_slots.insert(insert_at, edge_index);
    self.pending.insert(insert_at, VecDeque::new());
    if insert_at <= self.active_slot && self.edge_slots.len() > 1 {
      self.active_slot = self.active_slot.saturating_add(1);
    }
    Ok(insert_at)
  }

  fn pop_active_if_ready(&mut self) -> Option<In> {
    if self.active_slot >= self.pending.len() {
      return None;
    }
    if let Some(value) = self.pending[self.active_slot].pop_front() {
      return Some(value);
    }

    if !self.source_done {
      return None;
    }

    while self.active_slot < self.pending.len() && self.pending[self.active_slot].is_empty() {
      self.active_slot = self.active_slot.saturating_add(1);
    }
    if self.active_slot >= self.pending.len() {
      return None;
    }
    self.pending[self.active_slot].pop_front()
  }
}

impl<In> FlowLogic for ConcatLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    self.apply_with_edge(0, input)
  }

  fn apply_with_edge(&mut self, edge_index: usize, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.fan_in == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let slot = self.slot_for_edge(edge_index)?;
    self.pending[slot].push_back(value);

    if let Some(output) = self.pop_active_if_ready() {
      return Ok(vec![Box::new(output) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn expected_fan_in(&self) -> Option<usize> {
    Some(self.fan_in)
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if let Some(output) = self.pop_active_if_ready() {
      return Ok(vec![Box::new(output) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.edge_slots.clear();
    self.pending.clear();
    self.active_slot = 0;
    self.source_done = false;
    Ok(())
  }
}

impl<In, Out, F> GraphStage<In, Out, StreamNotUsed> for MapLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Out + Send + Sync + Clone + 'static,
{
  fn shape(&self) -> StreamShape<In, Out> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<In, Out, StreamNotUsed>> {
    Box::new(MapLogic { func: self.func.clone(), _pd: PhantomData })
  }
}

impl<In, Out, Mat2, F> GraphStage<In, Out, StreamNotUsed> for FlatMapConcatLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + Clone + 'static,
{
  fn shape(&self) -> StreamShape<In, Out> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<In, Out, StreamNotUsed>> {
    Box::new(FlatMapConcatLogic {
      func:          self.func.clone(),
      active_inner:  None,
      pending_outer: VecDeque::new(),
      _pd:           PhantomData,
    })
  }
}

impl<In, Out, Mat2, F> GraphStage<In, Out, StreamNotUsed> for FlatMapMergeLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + Clone + 'static,
{
  fn shape(&self) -> StreamShape<In, Out> {
    StreamShape::new(Inlet::new(), Outlet::new())
  }

  fn create_logic(&self) -> Box<dyn GraphStageLogic<In, Out, StreamNotUsed>> {
    Box::new(FlatMapMergeLogic {
      breadth:        self.breadth,
      func:           self.func.clone(),
      active_streams: VecDeque::new(),
      pending_outer:  VecDeque::new(),
      _pd:            PhantomData,
    })
  }
}

fn combine_mat<Left, Right, C>(left: Left, right: Right) -> C::Out
where
  C: MatCombineRule<Left, Right>, {
  C::combine(left, right)
}
