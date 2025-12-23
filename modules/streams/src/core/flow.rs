use alloc::{boxed::Box, vec, vec::Vec};
use core::{any::TypeId, marker::PhantomData};

use super::{
  DynValue, FlowDefinition, FlowLogic, Inlet, MatCombine, MatCombineRule, Outlet, Source, StageDefinition, StageKind,
  StreamError, StreamGraph, StreamNotUsed, StreamShape, StreamStage, downcast_value, graph_stage::GraphStage,
  graph_stage_logic::GraphStageLogic, sink::Sink, stage_context::StageContext,
};

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

  pub(crate) fn into_parts(self) -> (StreamGraph, Mat) {
    (self.graph, self.mat)
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

fn combine_mat<Left, Right, C>(left: Left, right: Right) -> C::Out
where
  C: MatCombineRule<Left, Right>, {
  C::combine(left, right)
}
