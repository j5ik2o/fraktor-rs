use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use core::{
  any::TypeId,
  future::Future,
  marker::PhantomData,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use super::{
  FlowDefinition, FlowMonitor, FlowSubFlow, MatCombine, MatCombineRule, OverflowStrategy, RestartBackoff,
  RestartSettings, Source, StageDefinition, StageKind, StreamDslError, StreamError, StreamGraph, StreamNotUsed,
  StreamStage, SupervisionStrategy,
  shape::{Inlet, Outlet, StreamShape},
  sink::Sink,
  validate_positive_argument,
};
use crate::core::Attributes;

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

  /// Creates a flow from a pre-built stream graph and materialized value.
  #[must_use]
  pub fn from_graph(graph: StreamGraph, mat: Mat) -> Self {
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

fn combine_mat<Left, Right, C>(left: Left, right: Right) -> C::Out
where
  C: MatCombineRule<Left, Right>, {
  C::combine(left, right)
}
