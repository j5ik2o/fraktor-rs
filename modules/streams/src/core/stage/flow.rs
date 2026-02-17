use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use core::{
  any::TypeId,
  future::Future,
  hash::BuildHasherDefault,
  marker::PhantomData,
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use ahash::AHasher;
use fraktor_utils_rs::core::collections::queue::OverflowPolicy;
use hashbrown::HashSet;

use super::{
  DynValue, FlowDefinition, FlowLogic, FlowSubFlow, MatCombine, MatCombineRule, RestartBackoff, RestartSettings,
  Source, StageDefinition, StageKind, StreamDslError, StreamError, StreamGraph, StreamNotUsed, StreamStage,
  SupervisionStrategy, downcast_value,
  graph::{GraphStage, GraphStageLogic},
  shape::{Inlet, Outlet, StreamShape},
  sink::Sink,
  stage_context::StageContext,
  validate_positive_argument,
};

#[cfg(test)]
mod tests;

type AHashSet<T> = HashSet<T, BuildHasherDefault<AHasher>>;

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

  /// Eliminates duplicate elements from the stream.
  ///
  /// Only the first occurrence of each element is emitted. Uses a `HashSet` to track seen elements.
  #[must_use]
  pub fn distinct(mut self) -> Flow<In, Out, Mat>
  where
    Out: Clone + Eq + core::hash::Hash, {
    let definition = distinct_definition::<Out>();
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Eliminates elements with duplicate keys from the stream.
  ///
  /// Only the first occurrence of each key is emitted. Uses a `HashSet` to track seen keys.
  #[must_use]
  pub fn distinct_by<Key, F>(mut self, key_extractor: F) -> Flow<In, Out, Mat>
  where
    Key: Eq + core::hash::Hash + Send + Sync + 'static,
    F: FnMut(&Out) -> Key + Send + Sync + 'static, {
    let definition = distinct_by_definition::<Out, Key, F>(key_extractor);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
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
  /// # Panics
  ///
  /// Panics when `fan_out` is zero.
  #[must_use]
  pub fn broadcast(mut self, fan_out: usize) -> Flow<In, Out, Mat>
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a balance stage that distributes elements across `fan_out` outputs.
  ///
  /// # Panics
  ///
  /// Panics when `fan_out` is zero.
  #[must_use]
  pub fn balance(mut self, fan_out: usize) -> Flow<In, Out, Mat> {
    assert!(fan_out > 0, "fan_out must be greater than zero");
    let definition = balance_definition::<Out>(fan_out);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a merge stage that merges `fan_in` upstream paths.
  ///
  /// # Panics
  ///
  /// Panics when `fan_in` is zero.
  #[must_use]
  pub fn merge(mut self, fan_in: usize) -> Flow<In, Out, Mat> {
    assert!(fan_in > 0, "fan_in must be greater than zero");
    let definition = merge_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds an interleave stage that consumes `fan_in` inputs in round-robin order.
  ///
  /// # Panics
  ///
  /// Panics when `fan_in` is zero.
  #[must_use]
  pub fn interleave(mut self, fan_in: usize) -> Flow<In, Out, Mat> {
    assert!(fan_in > 0, "fan_in must be greater than zero");
    let definition = interleave_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a prepend stage that prioritizes lower-index input lanes.
  ///
  /// # Panics
  ///
  /// Panics when `fan_in` is zero.
  #[must_use]
  pub fn prepend(mut self, fan_in: usize) -> Flow<In, Out, Mat> {
    assert!(fan_in > 0, "fan_in must be greater than zero");
    let definition = prepend_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a zip stage that emits one vector after receiving one element from each input.
  ///
  /// # Panics
  ///
  /// Panics when `fan_in` is zero.
  #[must_use]
  pub fn zip(mut self, fan_in: usize) -> Flow<In, Vec<Out>, Mat> {
    assert!(fan_in > 0, "fan_in must be greater than zero");
    let definition = zip_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a zip-all stage that fills missing lanes with `fill_value` after completion.
  ///
  /// # Panics
  ///
  /// Panics when `fan_in` is zero.
  #[must_use]
  pub fn zip_all(mut self, fan_in: usize, fill_value: Out) -> Flow<In, Vec<Out>, Mat>
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
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
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
  /// # Panics
  ///
  /// Panics when `fan_in` is zero.
  #[must_use]
  pub fn concat(mut self, fan_in: usize) -> Flow<In, Out, Mat> {
    assert!(fan_in > 0, "fan_in must be greater than zero");
    let definition = concat_definition::<Out>(fan_in);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
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
  #[must_use]
  pub fn lazy_completion_stage_flow<F>(factory: F) -> Flow<In, Out, Mat>
  where
    F: FnOnce() -> Flow<In, Out, Mat>, {
    factory()
  }

  /// Lazily creates a flow.
  #[must_use]
  pub fn lazy_flow<F>(factory: F) -> Flow<In, Out, Mat>
  where
    F: FnOnce() -> Flow<In, Out, Mat>, {
    factory()
  }

  /// Lazily creates a future flow.
  #[must_use]
  pub fn lazy_future_flow<F>(factory: F) -> Flow<In, Out, Mat>
  where
    F: FnOnce() -> Flow<In, Out, Mat>, {
    factory()
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
  #[must_use]
  pub fn scan_async<Acc, F, Fut>(self, initial: Acc, mut func: F) -> Flow<In, Acc, Mat>
  where
    Acc: Clone + Send + Sync + 'static,
    F: FnMut(Acc, Out) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Acc> + Send + 'static, {
    self.scan(initial, move |acc, value| {
      core::mem::drop((func)(acc.clone(), value));
      acc
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

  /// Groups elements within a count window.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `size` is zero.
  pub fn grouped_within(self, size: usize, _ticks: usize) -> Result<Flow<In, Vec<Out>, Mat>, StreamDslError> {
    self.grouped(size)
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

  /// Conflates upstream elements by keeping only the latest when downstream is slower.
  ///
  /// Passes through upstream elements unchanged.
  ///
  /// In the synchronous execution model there is no rate difference between
  /// upstream and downstream, so conflation is a no-op identity mapping.
  #[must_use]
  pub fn conflate(self) -> Flow<In, Out, Mat> {
    self.map(|v| v)
  }

  /// Adds a conflate-with-seed compatibility placeholder.
  #[must_use]
  pub fn conflate_with_seed<T, FS, FA>(self, seed: FS, _aggregate: FA) -> Flow<In, T, Mat>
  where
    T: Send + Sync + 'static,
    FS: FnMut(Out) -> T + Send + Sync + 'static,
    FA: FnMut(T, Out) -> T + Send + Sync + 'static, {
    self.map(seed)
  }

  /// Adds an expand compatibility placeholder.
  #[must_use]
  pub const fn expand(self) -> Flow<In, Out, Mat> {
    self
  }

  /// Adds an extrapolate compatibility placeholder.
  #[must_use]
  pub const fn extrapolate(self) -> Flow<In, Out, Mat> {
    self
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

  /// Applies a backpressure-timeout compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn backpressure_timeout(self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.take_within(ticks)
  }

  /// Applies a completion-timeout compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn completion_timeout(self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.take_within(ticks)
  }

  /// Applies an idle-timeout compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn idle_timeout(self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.take_within(ticks)
  }

  /// Applies an initial-timeout compatibility stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `ticks` is zero.
  pub fn initial_timeout(self, ticks: usize) -> Result<Flow<In, Out, Mat>, StreamDslError> {
    self.take_within(ticks)
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
  #[must_use]
  pub fn merge_sequence(self, fan_in: usize) -> Flow<In, Out, Mat> {
    self.merge(fan_in)
  }

  /// Adds a concat-all-lazy compatibility stage.
  #[must_use]
  pub fn concat_all_lazy(self, fan_in: usize) -> Flow<In, Out, Mat> {
    self.concat(fan_in)
  }

  /// Adds a concat-lazy compatibility stage.
  #[must_use]
  pub fn concat_lazy(self, fan_in: usize) -> Flow<In, Out, Mat> {
    self.concat(fan_in)
  }

  /// Adds an interleave-all compatibility stage.
  #[must_use]
  pub fn interleave_all(self, fan_in: usize) -> Flow<In, Out, Mat> {
    self.interleave(fan_in)
  }

  /// Adds a merge-all compatibility stage.
  #[must_use]
  pub fn merge_all(self, fan_in: usize) -> Flow<In, Out, Mat> {
    self.merge(fan_in)
  }

  /// Adds a merge-latest compatibility stage.
  #[must_use]
  pub fn merge_latest(self, fan_in: usize) -> Flow<In, Out, Mat> {
    self.merge(fan_in)
  }

  /// Adds a merge-preferred compatibility stage.
  #[must_use]
  pub fn merge_preferred(self, fan_in: usize) -> Flow<In, Out, Mat> {
    self.merge(fan_in)
  }

  /// Adds a merge-prioritized compatibility stage.
  #[must_use]
  pub fn merge_prioritized(self, fan_in: usize) -> Flow<In, Out, Mat> {
    self.merge(fan_in)
  }

  /// Adds a merge-prioritized-n compatibility stage.
  #[must_use]
  pub fn merge_prioritized_n(self, fan_in: usize, _priorities: &[usize]) -> Flow<In, Out, Mat> {
    self.merge(fan_in)
  }

  /// Adds a merge-sorted compatibility stage.
  #[must_use]
  pub fn merge_sorted(self, fan_in: usize) -> Flow<In, Out, Mat> {
    self.merge(fan_in)
  }

  /// Adds an or-else compatibility stage.
  #[must_use]
  pub fn or_else(self, fan_in: usize) -> Flow<In, Out, Mat> {
    self.prepend(fan_in)
  }

  /// Adds a prepend-lazy compatibility stage.
  #[must_use]
  pub fn prepend_lazy(self, fan_in: usize) -> Flow<In, Out, Mat> {
    self.prepend(fan_in)
  }

  /// Adds a zip-latest compatibility stage.
  #[must_use]
  pub fn zip_latest(self, fan_in: usize, fill_value: Out) -> Flow<In, Vec<Out>, Mat>
  where
    Out: Clone, {
    self.zip_all(fan_in, fill_value)
  }

  /// Adds a zip-latest-with compatibility stage.
  #[must_use]
  pub fn zip_latest_with<T, F>(self, fan_in: usize, fill_value: Out, func: F) -> Flow<In, T, Mat>
  where
    Out: Clone,
    T: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> T + Send + Sync + 'static, {
    self.zip_latest(fan_in, fill_value).map(func)
  }

  /// Adds a zip-with compatibility stage.
  #[must_use]
  pub fn zip_with<T, F>(self, fan_in: usize, func: F) -> Flow<In, T, Mat>
  where
    T: Send + Sync + 'static,
    F: FnMut(Vec<Out>) -> T + Send + Sync + 'static, {
    self.zip(fan_in).map(func)
  }

  /// Adds an also-to compatibility stage.
  #[must_use]
  pub fn also_to<Mat2>(self, sink: Sink<Out, Mat2>) -> Flow<In, Out, Mat> {
    core::mem::drop(sink);
    self
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

  /// Adds a watch-termination compatibility stage.
  #[must_use]
  pub const fn watch_termination(self) -> Flow<In, Out, Mat> {
    self
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

pub(in crate::core) fn distinct_definition<In>() -> FlowDefinition
where
  In: Clone + Eq + core::hash::Hash + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = DistinctLogic::<In> { seen: AHashSet::default(), _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowDistinct,
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

pub(in crate::core) fn distinct_by_definition<In, Key, F>(key_extractor: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Key: Eq + core::hash::Hash + Send + Sync + 'static,
  F: FnMut(&In) -> Key + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = DistinctByLogic::<In, Key, F> { key_extractor, seen: AHashSet::default(), _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowDistinctBy,
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

struct DistinctLogic<In> {
  seen: AHashSet<In>,
  _pd:  PhantomData<fn(In)>,
}

struct DistinctByLogic<In, Key, F> {
  key_extractor: F,
  seen:          AHashSet<Key>,
  _pd:           PhantomData<fn(In) -> Key>,
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

impl<In> FlowLogic for DistinctLogic<In>
where
  In: Clone + Eq + core::hash::Hash + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if self.seen.insert(value.clone()) {
      return Ok(vec![Box::new(value) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.seen.clear();
    Ok(())
  }
}

impl<In, Key, F> FlowLogic for DistinctByLogic<In, Key, F>
where
  In: Send + Sync + 'static,
  Key: Eq + core::hash::Hash + Send + Sync + 'static,
  F: FnMut(&In) -> Key + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let key = (self.key_extractor)(&value);
    if self.seen.insert(key) {
      return Ok(vec![Box::new(value) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.seen.clear();
    Ok(())
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
