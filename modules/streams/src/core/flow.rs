use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use core::{any::TypeId, marker::PhantomData};

use fraktor_utils_rs::core::collections::queue::OverflowPolicy;

use super::{
  DynValue, FlowDefinition, FlowLogic, Inlet, MatCombine, MatCombineRule, Outlet, Source, StageDefinition, StageKind,
  StreamError, StreamGraph, StreamNotUsed, StreamShape, StreamStage, downcast_value, graph_stage::GraphStage,
  graph_stage_logic::GraphStageLogic, sink::Sink, stage_context::StageContext,
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
  /// # Panics
  ///
  /// Panics when `breadth` is zero.
  #[must_use]
  pub fn flat_map_merge<T, Mat2, F>(mut self, breadth: usize, func: F) -> Flow<In, T, Mat>
  where
    T: Send + Sync + 'static,
    Mat2: Send + Sync + 'static,
    F: FnMut(Out) -> Source<T, Mat2> + Send + Sync + 'static, {
    assert!(breadth > 0, "breadth must be greater than zero");
    let definition = flat_map_merge_definition::<Out, T, Mat2, F>(breadth, func);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Adds a buffer stage with an overflow strategy.
  ///
  /// # Panics
  ///
  /// Panics when `capacity` is zero.
  #[must_use]
  pub fn buffer(mut self, capacity: usize, overflow_policy: OverflowPolicy) -> Flow<In, Out, Mat> {
    assert!(capacity > 0, "capacity must be greater than zero");
    let definition = buffer_definition::<Out>(capacity, overflow_policy);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
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

  /// Adds a group-by stage that annotates elements with their computed key.
  ///
  /// # Panics
  ///
  /// Panics when `max_substreams` is zero.
  #[must_use]
  pub fn group_by<K, F>(mut self, max_substreams: usize, key_fn: F) -> Flow<In, (K, Out), Mat>
  where
    K: Send + Sync + 'static,
    F: FnMut(&Out) -> K + Send + Sync + 'static, {
    assert!(max_substreams > 0, "max_substreams must be greater than zero");
    let definition = group_by_definition::<Out, K, F>(max_substreams, key_fn);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Splits the stream before elements matching `predicate`.
  #[must_use]
  pub fn split_when<F>(mut self, predicate: F) -> Flow<In, Vec<Out>, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = split_when_definition::<Out, F>(predicate);
    let inlet_id = definition.inlet;
    let from = self.graph.tail_outlet();
    self.graph.push_stage(StageDefinition::Flow(definition));
    if let Some(from) = from {
      let _ = self.graph.connect(&Outlet::<Out>::from_id(from), &Inlet::<Out>::from_id(inlet_id), MatCombine::KeepLeft);
    }
    Flow { graph: self.graph, mat: self.mat, _pd: PhantomData }
  }

  /// Splits the stream after elements matching `predicate`.
  #[must_use]
  pub fn split_after<F>(mut self, predicate: F) -> Flow<In, Vec<Out>, Mat>
  where
    F: FnMut(&Out) -> bool + Send + Sync + 'static, {
    let definition = split_after_definition::<Out, F>(predicate);
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

impl<In, Out, Mat> StreamStage for Flow<In, Out, Mat> {
  type In = In;
  type Out = Out;

  fn shape(&self) -> StreamShape<Self::In, Self::Out> {
    let inlet = self.graph.head_inlet().map(Inlet::from_id).unwrap_or_default();
    let outlet = self.graph.tail_outlet().map(Outlet::from_id).unwrap_or_default();
    StreamShape::new(inlet, outlet)
  }
}

pub(super) fn map_definition<In, Out, F>(func: F) -> FlowDefinition
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
    logic:       Box::new(logic),
  }
}

pub(super) fn flat_map_concat_definition<In, Out, Mat2, F>(func: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<Out> = Outlet::new();
  let logic = FlatMapConcatLogic { func, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowFlatMapConcat,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(logic),
  }
}

pub(super) fn flat_map_merge_definition<In, Out, Mat2, F>(breadth: usize, func: F) -> FlowDefinition
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
    waiting_streams: VecDeque::new(),
    _pd: PhantomData,
  };
  FlowDefinition {
    kind:        StageKind::FlowFlatMapMerge,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<Out>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(logic),
  }
}

pub(super) fn buffer_definition<In>(capacity: usize, overflow_policy: OverflowPolicy) -> FlowDefinition
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
    logic:       Box::new(logic),
  }
}

pub(super) fn async_boundary_definition<In>() -> FlowDefinition
where
  In: Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<In> = Outlet::new();
  let logic = AsyncBoundaryLogic::<In> { _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowAsyncBoundary,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<In>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(logic),
  }
}

pub(super) fn group_by_definition<In, Key, F>(max_substreams: usize, key_fn: F) -> FlowDefinition
where
  In: Send + Sync + 'static,
  Key: Send + Sync + 'static,
  F: FnMut(&In) -> Key + Send + Sync + 'static, {
  let inlet: Inlet<In> = Inlet::new();
  let outlet: Outlet<(Key, In)> = Outlet::new();
  let logic = GroupByLogic::<In, Key, F> { max_substreams, key_fn, _pd: PhantomData };
  FlowDefinition {
    kind:        StageKind::FlowGroupBy,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<In>(),
    output_type: TypeId::of::<(Key, In)>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(logic),
  }
}

pub(super) fn split_when_definition<In, F>(predicate: F) -> FlowDefinition
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
    logic:       Box::new(logic),
  }
}

pub(super) fn split_after_definition<In, F>(predicate: F) -> FlowDefinition
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
    logic:       Box::new(logic),
  }
}

pub(super) fn merge_substreams_definition<In>() -> FlowDefinition
where
  In: Send + Sync + 'static, {
  flatten_substreams_definition::<In>(StageKind::FlowMergeSubstreams)
}

pub(super) fn concat_substreams_definition<In>() -> FlowDefinition
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
    logic: Box::new(logic),
  }
}

pub(super) fn broadcast_definition<In>(fan_out: usize) -> FlowDefinition
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
    logic:       Box::new(logic),
  }
}

pub(super) fn balance_definition<In>(fan_out: usize) -> FlowDefinition
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
    logic:       Box::new(logic),
  }
}

pub(super) fn merge_definition<In>(fan_in: usize) -> FlowDefinition
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
    logic:       Box::new(logic),
  }
}

pub(super) fn zip_definition<In>(fan_in: usize) -> FlowDefinition
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
    logic:       Box::new(logic),
  }
}

pub(super) fn concat_definition<In>(fan_in: usize) -> FlowDefinition
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
    logic:       Box::new(logic),
  }
}

struct MapLogic<In, Out, F> {
  func: F,
  _pd:  PhantomData<fn(In) -> Out>,
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
  func: F,
  _pd:  PhantomData<fn(In) -> (Out, Mat2)>,
}

struct FlatMapMergeLogic<In, Out, Mat2, F> {
  breadth:         usize,
  func:            F,
  active_streams:  VecDeque<VecDeque<Out>>,
  waiting_streams: VecDeque<VecDeque<Out>>,
  _pd:             PhantomData<fn(In) -> (Out, Mat2)>,
}

struct BufferLogic<In> {
  capacity:        usize,
  overflow_policy: OverflowPolicy,
  pending:         VecDeque<In>,
  source_done:     bool,
}

struct AsyncBoundaryLogic<In> {
  _pd: PhantomData<fn(In)>,
}

struct GroupByLogic<In, Key, F> {
  max_substreams: usize,
  key_fn:         F,
  _pd:            PhantomData<fn(In) -> Key>,
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

impl<In, Out, Mat2, F> FlowLogic for FlatMapConcatLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let source = (self.func)(value);
    let outputs = source.collect_values()?;
    Ok(outputs.into_iter().map(|item| Box::new(item) as DynValue).collect())
  }
}

impl<In, Out, Mat2, F> FlatMapMergeLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn promote_waiting(&mut self) {
    while self.active_streams.len() < self.breadth {
      let Some(stream) = self.waiting_streams.pop_front() else {
        break;
      };
      self.active_streams.push_back(stream);
    }
  }

  fn enqueue_inner_stream(&mut self, value: In) -> Result<(), StreamError> {
    let source = (self.func)(value);
    let outputs = source.collect_values()?;
    if outputs.is_empty() {
      return Ok(());
    }
    let mut stream = VecDeque::with_capacity(outputs.len());
    stream.extend(outputs);
    self.waiting_streams.push_back(stream);
    self.promote_waiting();
    Ok(())
  }

  fn pop_next_value(&mut self) -> Option<Out> {
    self.promote_waiting();
    loop {
      let mut stream = self.active_streams.pop_front()?;
      let Some(value) = stream.pop_front() else {
        self.promote_waiting();
        continue;
      };
      if stream.is_empty() {
        self.promote_waiting();
      } else {
        self.active_streams.push_back(stream);
      }
      return Some(value);
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
    self.enqueue_inner_stream(value)?;
    if let Some(output) = self.pop_next_value() {
      return Ok(vec![Box::new(output) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if let Some(output) = self.pop_next_value() {
      return Ok(vec![Box::new(output) as DynValue]);
    }
    Ok(Vec::new())
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
    let source = (self.func)(value);
    if let Ok(mut outputs) = source.collect_values()
      && let Some(first) = outputs.pop()
    {
      ctx.push(first);
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
    if self.enqueue_inner_stream(value).is_err() {
      ctx.fail(StreamError::Failed);
      return;
    }
    if let Some(output) = self.pop_next_value() {
      ctx.push(output);
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
}

impl<In> FlowLogic for AsyncBoundaryLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    Ok(vec![input])
  }
}

impl<In, Key, F> FlowLogic for GroupByLogic<In, Key, F>
where
  In: Send + Sync + 'static,
  Key: Send + Sync + 'static,
  F: FnMut(&In) -> Key + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.max_substreams == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let key = (self.key_fn)(&value);
    Ok(vec![Box::new((key, value)) as DynValue])
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

struct ZipLogic<In> {
  fan_in:     usize,
  edge_slots: Vec<usize>,
  pending:    Vec<VecDeque<In>>,
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
    Box::new(FlatMapConcatLogic { func: self.func.clone(), _pd: PhantomData })
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
      breadth:         self.breadth,
      func:            self.func.clone(),
      active_streams:  VecDeque::new(),
      waiting_streams: VecDeque::new(),
      _pd:             PhantomData,
    })
  }
}

fn combine_mat<Left, Right, C>(left: Left, right: Right) -> C::Out
where
  C: MatCombineRule<Left, Right>, {
  C::combine(left, right)
}
