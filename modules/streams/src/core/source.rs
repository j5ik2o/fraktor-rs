use alloc::{boxed::Box, vec, vec::Vec};
use core::{any::TypeId, marker::PhantomData};

use super::{
  DynValue, FlowDefinition, Inlet, MatCombine, MatCombineRule, Materialized, Materializer, Outlet, RunnableGraph,
  SourceDefinition, SourceLogic, StageDefinition, StageKind, StreamError, StreamGraph, StreamNotUsed, StreamShape,
  StreamStage, downcast_value,
  flow::{
    balance_definition, broadcast_definition, concat_definition, flat_map_concat_definition, map_definition,
    merge_definition, zip_definition,
  },
  graph_stage::GraphStage,
  graph_stage_logic::GraphStageLogic,
  sink::Sink,
  stage_context::StageContext,
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
      logic:       Box::new(logic),
    };
    graph.push_stage(StageDefinition::Source(definition));
    Self { graph, mat: StreamNotUsed::new(), _pd: PhantomData }
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

  pub(crate) fn collect_values(self) -> Result<Vec<Out>, StreamError> {
    let (mut source, mut flows) = self.graph.into_source_parts()?;
    let mut outputs = Vec::new();
    while let Some(value) = source.logic.pull()? {
      let values = apply_flows(&mut flows, value)?;
      for value in values {
        let item = downcast_value::<Out>(value)?;
        outputs.push(item);
      }
    }
    Ok(outputs)
  }

  pub(crate) fn into_parts(self) -> (StreamGraph, Mat) {
    (self.graph, self.mat)
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

fn apply_flows(flows: &mut Vec<FlowDefinition>, value: DynValue) -> Result<Vec<DynValue>, StreamError> {
  let mut values = vec![value];
  for flow in flows {
    let mut next = Vec::new();
    for value in values {
      let outputs = flow.logic.apply(value)?;
      next.extend(outputs);
    }
    values = next;
  }
  Ok(values)
}

struct SingleSourceLogic<Out> {
  value: Option<Out>,
}

impl<Out> SourceLogic for SingleSourceLogic<Out>
where
  Out: Send + Sync + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(self.value.take().map(|value| Box::new(value) as DynValue))
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
