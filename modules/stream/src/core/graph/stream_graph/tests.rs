use core::any::TypeId;

use crate::core::{
  Attributes, DemandTracker, DynValue, MatCombine, SinkDecision, SinkDefinition, SinkLogic, SourceDefinition,
  SourceLogic, StageDefinition, StreamError,
  graph::StreamGraph,
  shape::{Inlet, Outlet, PortId},
  stage::{Source, StageKind},
};

impl StreamGraph {
  pub(in crate::core) fn attributes(&self) -> &Attributes {
    &self.attributes
  }

  fn stage_kinds(&self) -> Vec<StageKind> {
    self.nodes.iter().map(|node| node.stage.kind()).collect()
  }

  fn stage_mat_combines(&self) -> Vec<MatCombine> {
    self.nodes.iter().map(|node| node.stage.mat_combine()).collect()
  }

  fn connection_count(&self) -> usize {
    self.edges.len()
  }

  fn connections(&self) -> Vec<(PortId, PortId, MatCombine)> {
    self.edges.iter().map(|edge| (edge.from, edge.to, edge.mat)).collect()
  }

  fn attribute_names(&self) -> &[alloc::string::String] {
    self.attributes.names()
  }
}

#[test]
fn connect_rejects_unknown_ports() {
  let mut graph = StreamGraph::new();
  let result = graph.connect(&Outlet::<u32>::new(), &Inlet::<u32>::new(), MatCombine::KeepLeft);
  assert!(result.is_err());
}

#[test]
fn graph_tracks_stage_metadata() {
  let source = Source::single(1_u32).map(|value| value + 1);
  let (graph, _mat) = source.into_parts();
  assert_eq!(graph.stage_kinds(), vec![StageKind::SourceSingle, StageKind::FlowMap]);
  assert_eq!(graph.stage_mat_combines(), vec![MatCombine::KeepRight, MatCombine::KeepLeft]);
  assert_eq!(graph.connection_count(), 1);
  assert_eq!(graph.connections().len(), 1);
}

#[derive(Default)]
struct EmptySourceLogic;

impl SourceLogic for EmptySourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(None)
  }
}

struct IgnoreSinkLogic;

impl SinkLogic for IgnoreSinkLogic {
  fn on_start(&mut self, _demand: &mut DemandTracker) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_push(&mut self, _input: DynValue, _demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_error(&mut self, _error: StreamError) {}
}

#[test]
fn into_plan_allows_multiple_source_and_sink_nodes() {
  let source1_outlet: Outlet<u32> = Outlet::new();
  let source2_outlet: Outlet<u32> = Outlet::new();
  let sink1_inlet: Inlet<u32> = Inlet::new();
  let sink2_inlet: Inlet<u32> = Inlet::new();

  let source1 = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source1_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    supervision: crate::core::SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(EmptySourceLogic),
  };
  let source2 = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source2_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    supervision: crate::core::SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(EmptySourceLogic),
  };
  let sink1 = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink1_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    supervision: crate::core::SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(IgnoreSinkLogic),
  };
  let sink2 = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink2_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    supervision: crate::core::SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(IgnoreSinkLogic),
  };

  let mut graph = StreamGraph::new();
  graph.push_stage(StageDefinition::Source(source1));
  graph.push_stage(StageDefinition::Source(source2));
  graph.push_stage(StageDefinition::Sink(sink1));
  graph.push_stage(StageDefinition::Sink(sink2));

  assert!(graph.connect(&source1_outlet, &sink1_inlet, MatCombine::KeepLeft).is_ok());
  assert!(graph.connect(&source2_outlet, &sink2_inlet, MatCombine::KeepRight).is_ok());

  assert!(graph.into_plan().is_ok());
}

#[test]
fn graph_tracks_attributes() {
  let mut graph = StreamGraph::new();
  graph.set_attributes(Attributes::named("base"));
  graph.add_attributes(Attributes::named("extra"));

  assert_eq!(graph.attribute_names(), &[alloc::string::String::from("base"), alloc::string::String::from("extra")]);
}
