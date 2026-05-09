use alloc::string::String;
use core::any::TypeId;

use crate::{
  DemandTracker, DynValue, MatCombine, SinkDecision, SinkDefinition, SinkLogic, SourceDefinition, SourceLogic,
  StageDefinition, StreamError, SupervisionStrategy,
  attributes::{AsyncBoundaryAttr, Attributes, DispatcherAttribute},
  dsl::{Sink, Source},
  r#impl::stream_graph::StreamGraph,
  shape::{Inlet, Outlet, PortId},
  stage::StageKind,
};

impl StreamGraph {
  pub(crate) fn attributes(&self) -> &Attributes {
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

  fn attribute_names(&self) -> &[String] {
    self.attributes.names()
  }
}

#[test]
fn connect_rejects_unknown_ports() {
  let mut graph = StreamGraph::new();
  let result = graph.connect(&Outlet::<u32>::new(), &Inlet::<u32>::new(), MatCombine::Left);
  assert!(result.is_err());
}

#[test]
fn graph_tracks_stage_metadata() {
  let source = Source::single(1_u32).map(|value| value + 1);
  let (graph, _mat) = source.into_parts();
  assert_eq!(graph.stage_kinds(), vec![StageKind::SourceSingle, StageKind::FlowMap]);
  assert_eq!(graph.stage_mat_combines(), vec![MatCombine::Right, MatCombine::Left]);
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
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(EmptySourceLogic),
    attributes:  Attributes::new(),
  };
  let source2 = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source2_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(EmptySourceLogic),
    attributes:  Attributes::new(),
  };
  let sink1 = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink1_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(IgnoreSinkLogic),
    attributes:  Attributes::new(),
  };
  let sink2 = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink2_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::Left,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(IgnoreSinkLogic),
    attributes:  Attributes::new(),
  };

  let mut graph = StreamGraph::new();
  graph.push_stage(StageDefinition::Source(source1));
  graph.push_stage(StageDefinition::Source(source2));
  graph.push_stage(StageDefinition::Sink(sink1));
  graph.push_stage(StageDefinition::Sink(sink2));

  assert!(graph.connect(&source1_outlet, &sink1_inlet, MatCombine::Left).is_ok());
  assert!(graph.connect(&source2_outlet, &sink2_inlet, MatCombine::Right).is_ok());

  assert!(graph.into_plan().is_ok());
}

#[test]
fn graph_tracks_attributes() {
  let mut graph = StreamGraph::new();
  graph.set_attributes(Attributes::named("base"));
  graph.add_attributes(Attributes::named("extra"));

  assert_eq!(graph.attribute_names(), &[String::from("base"), String::from("extra")]);
}

// --- B-1: Per-node attribute infrastructure ---

#[test]
fn mark_last_node_async_sets_async_boundary_on_last_node() {
  // Given: a graph with a source node
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(EmptySourceLogic),
    attributes:  Attributes::new(),
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(IgnoreSinkLogic),
    attributes:  Attributes::new(),
  };

  let mut graph = StreamGraph::new();
  graph.push_stage(StageDefinition::Source(source));
  graph.push_stage(StageDefinition::Sink(sink));

  // When: marking the last node as async
  graph.mark_last_node_async();

  // Then: connect and build plan; the sink stage (last node) should have async attribute
  assert!(graph.connect(&source_outlet, &sink_inlet, MatCombine::Left).is_ok());
  let plan = graph.into_plan().expect("into_plan");
  let last_stage_attrs = plan.stages[1].attributes();
  assert!(last_stage_attrs.is_async());
  assert!(last_stage_attrs.get::<AsyncBoundaryAttr>().is_some());
}

#[test]
fn mark_last_node_async_is_noop_on_empty_graph() {
  // Given: an empty graph
  let mut graph = StreamGraph::new();

  // When: marking (no nodes exist)
  graph.mark_last_node_async();

  // Then: no panic; graph remains valid (though empty)
  assert!(graph.into_plan().is_err()); // empty graph → error is expected
}

#[test]
fn mark_last_node_dispatcher_sets_both_async_and_dispatcher() {
  // Given: a source → sink graph
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(EmptySourceLogic),
    attributes:  Attributes::new(),
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(IgnoreSinkLogic),
    attributes:  Attributes::new(),
  };

  let mut graph = StreamGraph::new();
  graph.push_stage(StageDefinition::Source(source));
  graph.push_stage(StageDefinition::Sink(sink));

  // When: marking last node with dispatcher
  graph.mark_last_node_dispatcher("my-dispatcher");

  // Then: the sink stage has both AsyncBoundaryAttr and DispatcherAttribute
  assert!(graph.connect(&source_outlet, &sink_inlet, MatCombine::Left).is_ok());
  let plan = graph.into_plan().expect("into_plan");
  let attrs = plan.stages[1].attributes();
  assert!(attrs.is_async());
  assert!(attrs.get::<DispatcherAttribute>().is_some());
  assert_eq!(attrs.get::<DispatcherAttribute>().unwrap().name(), "my-dispatcher");
}

#[test]
fn into_plan_transfers_node_attributes_to_stage_definitions() {
  // Given: a source → sink graph where the source has async boundary
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(EmptySourceLogic),
    attributes:  Attributes::new(),
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(IgnoreSinkLogic),
    attributes:  Attributes::new(),
  };

  let mut graph = StreamGraph::new();
  graph.push_stage(StageDefinition::Source(source));

  // When: marking the source node (first and currently last) as async before adding sink
  graph.mark_last_node_async();

  graph.push_stage(StageDefinition::Sink(sink));
  assert!(graph.connect(&source_outlet, &sink_inlet, MatCombine::Left).is_ok());
  let plan = graph.into_plan().expect("into_plan");

  // Then: the source stage has async attribute, sink does not
  assert!(plan.stages[0].attributes().is_async());
  assert!(!plan.stages[1].attributes().is_async());
}

#[test]
fn node_attributes_survive_graph_append() {
  // Given: source.async() → map, then append a sink to make a complete pipeline
  let (mut combined_graph, _) = Source::single(1_u32).r#async().map(|x: u32| x + 1).into_parts();
  let (sink_graph, _) = Sink::<u32, _>::ignore().into_parts();
  combined_graph.append(sink_graph);

  // When: converting to plan
  let plan = combined_graph.into_plan().expect("into_plan");

  // Then: the source stage (index 0) retains its async attribute
  assert!(plan.stages[0].attributes().is_async());

  // The map flow stage should NOT have async boundary
  assert!(!plan.stages[1].attributes().is_async());
}

#[test]
fn stage_definition_attributes_returns_empty_by_default() {
  // Given: a stage definition with no attributes set
  let source_outlet: Outlet<u32> = Outlet::new();
  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(EmptySourceLogic),
    attributes:  Attributes::new(),
  };
  let stage = StageDefinition::Source(source);

  // Then: attributes are empty
  assert!(stage.attributes().is_empty());
  assert!(!stage.attributes().is_async());
}

#[test]
fn stage_definition_with_attributes_sets_attributes() {
  // Given: a stage definition with no attributes
  let source_outlet: Outlet<u32> = Outlet::new();
  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(EmptySourceLogic),
    attributes:  Attributes::new(),
  };
  let stage = StageDefinition::Source(source);

  // When: setting async boundary attributes
  let stage = stage.with_attributes(Attributes::async_boundary());

  // Then: attributes contain async boundary
  assert!(stage.attributes().is_async());
  assert!(stage.attributes().get::<AsyncBoundaryAttr>().is_some());
}
