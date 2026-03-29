use core::any::TypeId;

use crate::core::{
  Attributes, DemandTracker, DynValue, MatCombine, SinkDecision, SinkDefinition, SinkLogic, SourceDefinition,
  SourceLogic, StageDefinition, StreamError, SupervisionStrategy,
  graph::island_splitter::IslandSplitter,
  shape::{Inlet, Outlet},
  stage::StageKind,
};

// --- Test helpers ---

#[derive(Default)]
struct EmptySourceLogic;

impl SourceLogic for EmptySourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(None)
  }
}

struct PassthroughFlowLogic;

impl crate::core::FlowLogic for PassthroughFlowLogic {
  fn apply(&mut self, input: DynValue) -> Result<alloc::vec::Vec<DynValue>, StreamError> {
    Ok(alloc::vec![input])
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

fn make_source(outlet: &Outlet<u32>, attrs: Attributes) -> StageDefinition {
  StageDefinition::Source(SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(EmptySourceLogic),
    attributes:  attrs,
  })
}

fn make_flow(inlet: &Inlet<u32>, outlet: &Outlet<u32>, attrs: Attributes) -> StageDefinition {
  StageDefinition::Flow(crate::core::FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       inlet.id(),
    outlet:      outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::Left,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(PassthroughFlowLogic),
    attributes:  attrs,
  })
}

fn make_sink(inlet: &Inlet<u32>, attrs: Attributes) -> StageDefinition {
  StageDefinition::Sink(SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::Right,
    supervision: SupervisionStrategy::Stop,
    restart:     None,
    logic:       Box::new(IgnoreSinkLogic),
    attributes:  attrs,
  })
}

fn build_plan(
  stages: alloc::vec::Vec<StageDefinition>,
  edges: alloc::vec::Vec<(crate::core::shape::PortId, crate::core::shape::PortId, MatCombine)>,
) -> crate::core::StreamPlan {
  crate::core::StreamPlan::from_parts(stages, edges).expect("build_plan")
}

// --- No async boundary: single island ---

#[test]
fn split_no_async_boundary_returns_single_island() {
  // Given: source → flow → sink, no async boundary
  let s_out: Outlet<u32> = Outlet::new();
  let f_in: Inlet<u32> = Inlet::new();
  let f_out: Outlet<u32> = Outlet::new();
  let k_in: Inlet<u32> = Inlet::new();

  let stages = alloc::vec![
    make_source(&s_out, Attributes::new()),
    make_flow(&f_in, &f_out, Attributes::new()),
    make_sink(&k_in, Attributes::new()),
  ];
  let edges = alloc::vec![(s_out.id(), f_in.id(), MatCombine::Left), (f_out.id(), k_in.id(), MatCombine::Left),];
  let plan = build_plan(stages, edges);

  // When: splitting
  let island_plan = IslandSplitter::split(plan);

  // Then: single island containing all 3 stages
  assert_eq!(island_plan.islands().len(), 1);
  assert!(island_plan.crossings().is_empty());
  assert_eq!(island_plan.islands()[0].stage_count(), 3);
}

// --- One async boundary: two islands ---

#[test]
fn split_one_async_at_source_creates_two_islands() {
  // Given: source(async) → flow → sink
  // Semantics: async on source means source is last in island 1; flow+sink in island 2
  let s_out: Outlet<u32> = Outlet::new();
  let f_in: Inlet<u32> = Inlet::new();
  let f_out: Outlet<u32> = Outlet::new();
  let k_in: Inlet<u32> = Inlet::new();

  let stages = alloc::vec![
    make_source(&s_out, Attributes::async_boundary()),
    make_flow(&f_in, &f_out, Attributes::new()),
    make_sink(&k_in, Attributes::new()),
  ];
  let edges = alloc::vec![(s_out.id(), f_in.id(), MatCombine::Left), (f_out.id(), k_in.id(), MatCombine::Left),];
  let plan = build_plan(stages, edges);

  // When: splitting
  let island_plan = IslandSplitter::split(plan);

  // Then: 2 islands, 1 crossing
  assert_eq!(island_plan.islands().len(), 2);
  assert_eq!(island_plan.crossings().len(), 1);

  // Island 1: [source], Island 2: [flow, sink]
  assert_eq!(island_plan.islands()[0].stage_count(), 1);
  assert_eq!(island_plan.islands()[1].stage_count(), 2);
}

#[test]
fn split_one_async_at_flow_creates_two_islands() {
  // Given: source → flow(async) → sink
  // Semantics: flow is last in island 1; sink in island 2
  let s_out: Outlet<u32> = Outlet::new();
  let f_in: Inlet<u32> = Inlet::new();
  let f_out: Outlet<u32> = Outlet::new();
  let k_in: Inlet<u32> = Inlet::new();

  let stages = alloc::vec![
    make_source(&s_out, Attributes::new()),
    make_flow(&f_in, &f_out, Attributes::async_boundary()),
    make_sink(&k_in, Attributes::new()),
  ];
  let edges = alloc::vec![(s_out.id(), f_in.id(), MatCombine::Left), (f_out.id(), k_in.id(), MatCombine::Left),];
  let plan = build_plan(stages, edges);

  // When: splitting
  let island_plan = IslandSplitter::split(plan);

  // Then: 2 islands, 1 crossing
  assert_eq!(island_plan.islands().len(), 2);
  assert_eq!(island_plan.crossings().len(), 1);

  // Island 1: [source, flow], Island 2: [sink]
  assert_eq!(island_plan.islands()[0].stage_count(), 2);
  assert_eq!(island_plan.islands()[1].stage_count(), 1);
}

// --- Two async boundaries: three islands ---

#[test]
fn split_two_async_boundaries_creates_three_islands() {
  // Given: source(async) → flow(async) → sink
  let s_out: Outlet<u32> = Outlet::new();
  let f_in: Inlet<u32> = Inlet::new();
  let f_out: Outlet<u32> = Outlet::new();
  let k_in: Inlet<u32> = Inlet::new();

  let stages = alloc::vec![
    make_source(&s_out, Attributes::async_boundary()),
    make_flow(&f_in, &f_out, Attributes::async_boundary()),
    make_sink(&k_in, Attributes::new()),
  ];
  let edges = alloc::vec![(s_out.id(), f_in.id(), MatCombine::Left), (f_out.id(), k_in.id(), MatCombine::Left),];
  let plan = build_plan(stages, edges);

  // When: splitting
  let island_plan = IslandSplitter::split(plan);

  // Then: 3 islands, 2 crossings
  assert_eq!(island_plan.islands().len(), 3);
  assert_eq!(island_plan.crossings().len(), 2);

  // Island 1: [source], Island 2: [flow], Island 3: [sink]
  assert_eq!(island_plan.islands()[0].stage_count(), 1);
  assert_eq!(island_plan.islands()[1].stage_count(), 1);
  assert_eq!(island_plan.islands()[2].stage_count(), 1);
}

// --- Island IDs are sequential ---

#[test]
fn split_assigns_sequential_island_ids() {
  // Given: source(async) → flow → sink → 2 islands
  let s_out: Outlet<u32> = Outlet::new();
  let f_in: Inlet<u32> = Inlet::new();
  let f_out: Outlet<u32> = Outlet::new();
  let k_in: Inlet<u32> = Inlet::new();

  let stages = alloc::vec![
    make_source(&s_out, Attributes::async_boundary()),
    make_flow(&f_in, &f_out, Attributes::new()),
    make_sink(&k_in, Attributes::new()),
  ];
  let edges = alloc::vec![(s_out.id(), f_in.id(), MatCombine::Left), (f_out.id(), k_in.id(), MatCombine::Left),];
  let plan = build_plan(stages, edges);

  let island_plan = IslandSplitter::split(plan);

  // Then: island IDs start at 0 and increment
  assert_eq!(island_plan.islands()[0].id().as_usize(), 0);
  assert_eq!(island_plan.islands()[1].id().as_usize(), 1);
}

// --- Crossing identifies correct edge ---

#[test]
fn split_crossing_identifies_upstream_and_downstream_islands() {
  // Given: source(async) → flow → sink
  let s_out: Outlet<u32> = Outlet::new();
  let f_in: Inlet<u32> = Inlet::new();
  let f_out: Outlet<u32> = Outlet::new();
  let k_in: Inlet<u32> = Inlet::new();

  let stages = alloc::vec![
    make_source(&s_out, Attributes::async_boundary()),
    make_flow(&f_in, &f_out, Attributes::new()),
    make_sink(&k_in, Attributes::new()),
  ];
  let edges = alloc::vec![(s_out.id(), f_in.id(), MatCombine::Left), (f_out.id(), k_in.id(), MatCombine::Left),];
  let plan = build_plan(stages, edges);

  let island_plan = IslandSplitter::split(plan);

  // Then: the crossing connects island 0 → island 1
  assert_eq!(island_plan.crossings().len(), 1);
  let crossing = &island_plan.crossings()[0];
  assert_eq!(crossing.from_island().as_usize(), 0);
  assert_eq!(crossing.to_island().as_usize(), 1);
  assert_eq!(crossing.from_port(), s_out.id());
  assert_eq!(crossing.to_port(), f_in.id());
}

// --- Dispatcher attribute ---

#[test]
fn split_transfers_dispatcher_attribute_to_island() {
  // Given: source → flow(async + dispatcher) → sink
  let s_out: Outlet<u32> = Outlet::new();
  let f_in: Inlet<u32> = Inlet::new();
  let f_out: Outlet<u32> = Outlet::new();
  let k_in: Inlet<u32> = Inlet::new();

  let dispatcher_attrs = Attributes::async_boundary().and(Attributes::dispatcher("custom-dispatcher"));

  let stages = alloc::vec![
    make_source(&s_out, Attributes::new()),
    make_flow(&f_in, &f_out, dispatcher_attrs),
    make_sink(&k_in, Attributes::new()),
  ];
  let edges = alloc::vec![(s_out.id(), f_in.id(), MatCombine::Left), (f_out.id(), k_in.id(), MatCombine::Left),];
  let plan = build_plan(stages, edges);

  let island_plan = IslandSplitter::split(plan);

  // Then: dispatcher は async 境界の後続 island に付与される
  assert_eq!(island_plan.islands()[0].dispatcher(), None);
  assert_eq!(island_plan.islands()[1].dispatcher(), Some("custom-dispatcher"));
}

#[test]
fn split_async_cut_preserves_unrelated_branch_in_downstream_island() {
  let source_a_out: Outlet<u32> = Outlet::new();
  let async_in: Inlet<u32> = Inlet::new();
  let async_out: Outlet<u32> = Outlet::new();
  let source_b_out: Outlet<u32> = Outlet::new();
  let merge_in: Inlet<u32> = Inlet::new();
  let merge_out: Outlet<u32> = Outlet::new();
  let sink_in: Inlet<u32> = Inlet::new();

  let stages = alloc::vec![
    make_source(&source_a_out, Attributes::new()),
    make_flow(&async_in, &async_out, Attributes::async_boundary().and(Attributes::dispatcher("branch-dispatcher"))),
    make_source(&source_b_out, Attributes::new()),
    StageDefinition::Flow(crate::core::FlowDefinition {
      kind:        StageKind::FlowMerge,
      inlet:       merge_in.id(),
      outlet:      merge_out.id(),
      input_type:  TypeId::of::<u32>(),
      output_type: TypeId::of::<u32>(),
      mat_combine: MatCombine::Left,
      supervision: SupervisionStrategy::Stop,
      restart:     None,
      logic:       Box::new(PassthroughFlowLogic),
      attributes:  Attributes::new(),
    }),
    make_sink(&sink_in, Attributes::new()),
  ];
  let edges = alloc::vec![
    (source_a_out.id(), async_in.id(), MatCombine::Left),
    (async_out.id(), merge_in.id(), MatCombine::Left),
    (source_b_out.id(), merge_in.id(), MatCombine::Left),
    (merge_out.id(), sink_in.id(), MatCombine::Left),
  ];
  let plan = build_plan(stages, edges);

  let island_plan = IslandSplitter::split(plan);

  assert_eq!(island_plan.islands().len(), 2);
  assert_eq!(island_plan.crossings().len(), 1);
  assert_eq!(island_plan.crossings()[0].from_island().as_usize(), 0);
  assert_eq!(island_plan.crossings()[0].to_island().as_usize(), 1);
  assert_eq!(island_plan.islands()[0].stage_count(), 2);
  assert_eq!(island_plan.islands()[1].stage_count(), 3);
  assert_eq!(island_plan.islands()[1].dispatcher(), Some("branch-dispatcher"));
}

// --- Longer pipeline ---

#[test]
fn split_four_stage_pipeline_with_middle_async() {
  // Given: source → flow1 → flow2(async) → sink
  let s_out: Outlet<u32> = Outlet::new();
  let f1_in: Inlet<u32> = Inlet::new();
  let f1_out: Outlet<u32> = Outlet::new();
  let f2_in: Inlet<u32> = Inlet::new();
  let f2_out: Outlet<u32> = Outlet::new();
  let k_in: Inlet<u32> = Inlet::new();

  let stages = alloc::vec![
    make_source(&s_out, Attributes::new()),
    make_flow(&f1_in, &f1_out, Attributes::new()),
    make_flow(&f2_in, &f2_out, Attributes::async_boundary()),
    make_sink(&k_in, Attributes::new()),
  ];
  let edges = alloc::vec![
    (s_out.id(), f1_in.id(), MatCombine::Left),
    (f1_out.id(), f2_in.id(), MatCombine::Left),
    (f2_out.id(), k_in.id(), MatCombine::Left),
  ];
  let plan = build_plan(stages, edges);

  let island_plan = IslandSplitter::split(plan);

  // Then: 2 islands
  // Island 1: [source, flow1, flow2], Island 2: [sink]
  assert_eq!(island_plan.islands().len(), 2);
  assert_eq!(island_plan.islands()[0].stage_count(), 3);
  assert_eq!(island_plan.islands()[1].stage_count(), 1);
}

// --- Each island has valid topological order ---

#[test]
fn split_each_island_has_valid_topological_order() {
  // Given: source(async) → flow → sink
  let s_out: Outlet<u32> = Outlet::new();
  let f_in: Inlet<u32> = Inlet::new();
  let f_out: Outlet<u32> = Outlet::new();
  let k_in: Inlet<u32> = Inlet::new();

  let stages = alloc::vec![
    make_source(&s_out, Attributes::async_boundary()),
    make_flow(&f_in, &f_out, Attributes::new()),
    make_sink(&k_in, Attributes::new()),
  ];
  let edges = alloc::vec![(s_out.id(), f_in.id(), MatCombine::Left), (f_out.id(), k_in.id(), MatCombine::Left),];
  let plan = build_plan(stages, edges);

  let island_plan = IslandSplitter::split(plan);

  // Then: each island has source and sink indices populated
  let island1 = &island_plan.islands()[0];
  assert!(!island1.source_indices().is_empty());
  // Island 1 has only a source, no sink (the crossing acts as sink)

  let island2 = &island_plan.islands()[1];
  assert!(!island2.sink_indices().is_empty());
}

// --- SingleIslandPlan::into_stream_plan ---

#[test]
fn single_island_plan_into_stream_plan_preserves_stages() {
  // Given: a plan that splits into a single island (no async boundaries)
  let s_out: Outlet<u32> = Outlet::new();
  let f_in: Inlet<u32> = Inlet::new();
  let f_out: Outlet<u32> = Outlet::new();
  let k_in: Inlet<u32> = Inlet::new();

  let stages = alloc::vec![
    make_source(&s_out, Attributes::new()),
    make_flow(&f_in, &f_out, Attributes::new()),
    make_sink(&k_in, Attributes::new()),
  ];
  let edges = alloc::vec![(s_out.id(), f_in.id(), MatCombine::Left), (f_out.id(), k_in.id(), MatCombine::Left),];
  let plan = build_plan(stages, edges);
  let island_plan = IslandSplitter::split(plan);

  // When: converting the single island back to a StreamPlan
  assert_eq!(island_plan.islands().len(), 1);
  let stream_plan = island_plan.into_single_plan();

  // Then: the StreamPlan has the same number of stages
  assert_eq!(stream_plan.stages.len(), 3);
  assert_eq!(stream_plan.edges.len(), 2);
}

#[test]
fn single_island_plan_into_stream_plan_preserves_source_and_sink_indices() {
  // Given: a plan that splits into a single island
  let s_out: Outlet<u32> = Outlet::new();
  let k_in: Inlet<u32> = Inlet::new();

  let stages = alloc::vec![make_source(&s_out, Attributes::new()), make_sink(&k_in, Attributes::new()),];
  let edges = alloc::vec![(s_out.id(), k_in.id(), MatCombine::Left),];
  let plan = build_plan(stages, edges);
  let island_plan = IslandSplitter::split(plan);

  // When: converting back
  let stream_plan = island_plan.into_single_plan();

  // Then: source and sink indices are preserved
  assert_eq!(stream_plan.source_indices.len(), 1);
  assert_eq!(stream_plan.sink_indices.len(), 1);
}

// --- IslandCrossing accessors ---

#[test]
fn island_crossing_mat_is_preserved() {
  // Given: a plan with an async boundary
  let s_out: Outlet<u32> = Outlet::new();
  let k_in: Inlet<u32> = Inlet::new();

  let stages = alloc::vec![make_source(&s_out, Attributes::async_boundary()), make_sink(&k_in, Attributes::new()),];
  let edges = alloc::vec![(s_out.id(), k_in.id(), MatCombine::Right),];
  let plan = build_plan(stages, edges);

  // When: splitting
  let island_plan = IslandSplitter::split(plan);

  // Then: crossing preserves the original mat combine
  assert_eq!(island_plan.crossings().len(), 1);
  let crossing = &island_plan.crossings()[0];
  assert_eq!(crossing.mat(), MatCombine::Right);
}
