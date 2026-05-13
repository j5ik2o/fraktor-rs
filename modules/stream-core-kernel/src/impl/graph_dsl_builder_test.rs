use crate::{
  dsl::{Flow, GraphDsl, GraphDslBuilder as PublicGraphDslBuilder, Sink, Source},
  r#impl::{
    fusing::StreamBufferConfig, graph_dsl_builder::GraphDslBuilder, interpreter::graph_interpreter::GraphInterpreter,
    materialization::StreamState,
  },
  materialization::{Completion, DriveOutcome, KeepBoth, KeepRight, StreamNotUsed},
};

fn drive_to_terminal(interpreter: &mut GraphInterpreter) {
  interpreter.start().expect("start");

  let mut idle_budget = 1024_usize;
  let mut drive_budget = 16384_usize;
  while interpreter.state() == StreamState::Running {
    assert!(drive_budget > 0, "stream did not reach terminal state within drive budget");
    drive_budget = drive_budget.saturating_sub(1);
    match interpreter.drive() {
      | DriveOutcome::Progressed => idle_budget = 1024,
      | DriveOutcome::Idle => {
        assert!(idle_budget > 0, "stream stalled");
        idle_budget = idle_budget.saturating_sub(1);
      },
    }
  }
}

#[test]
fn build_creates_flow_from_builder() {
  let flow = GraphDslBuilder::<u32, u32, StreamNotUsed>::new().via(Flow::new().map(|value| value + 1)).build();
  let graph = Source::single(1_u32).via(flow).into_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());

  drive_to_terminal(&mut interpreter);

  assert_eq!(completion.value(), Completion::Ready(Ok(2_u32)));
}

#[test]
fn graph_dsl_facade_creates_builder() {
  let flow = GraphDsl::builder::<u32>().via(Flow::new().map(|value| value * 2)).build();
  let graph = Source::single(3_u32).via(flow).into_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());

  drive_to_terminal(&mut interpreter);

  assert_eq!(completion.value(), Completion::Ready(Ok(6_u32)));
}

#[test]
fn graph_dsl_from_flow_maps_materialized_value() {
  let flow = GraphDsl::from_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|value| value + 1))
    .map_materialized_value(|_| 7_u32)
    .via(Flow::new().map(|value| value * 2))
    .build();
  let graph = Source::single(2_u32).via_mat(flow, KeepRight).into_mat(Sink::head(), KeepBoth);
  let (plan, (mat, completion)) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());

  drive_to_terminal(&mut interpreter);

  assert_eq!(mat, 7_u32);
  assert_eq!(completion.value(), Completion::Ready(Ok(6_u32)));
}

#[test]
fn to_mat_keeps_sink_materialized_value_rule() {
  let sink = GraphDslBuilder::<u32, u32, StreamNotUsed>::new()
    .via(Flow::new().map(|value| value + 3))
    .into_mat(Sink::head(), KeepRight);
  let graph = Source::single(4_u32).into_mat(sink, KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());

  drive_to_terminal(&mut interpreter);

  assert_eq!(completion.value(), Completion::Ready(Ok(7_u32)));
}

#[test]
fn from_flow_and_build_accept_non_sync_output_types() {
  let flow = Flow::<u32, u32, StreamNotUsed>::new().map(Source::single);
  let _rebuilt: Flow<u32, Source<u32, StreamNotUsed>, StreamNotUsed> = GraphDslBuilder::from_flow(flow).build();
}

// ---------------------------------------------------------------------------
// C4: GraphDSL.Builder — add_source / add_flow / add_sink / connect
// ---------------------------------------------------------------------------

#[test]
fn add_source_returns_outlet() {
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let outlet = builder.add_source(Source::single(42_u32)).unwrap();

  // Outlet should have a valid port id (non-default).
  // We verify that we can use it without panic.
  let _ = outlet.id();
}

#[test]
fn add_flow_returns_inlet_outlet_pair() {
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let (inlet, outlet) = builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|v| v + 1)).unwrap();

  let _ = inlet.id();
  let _ = outlet.id();
}

#[test]
fn add_sink_returns_inlet() {
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let inlet = builder.add_sink(Sink::<u32, _>::ignore()).unwrap();

  let _ = inlet.id();
}

#[test]
fn connect_wires_outlet_to_inlet() {
  // Given: a builder with a source and a sink added
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_outlet = builder.add_source(Source::single(7_u32)).unwrap();
  let sink_inlet = builder.add_sink(Sink::<u32, _>::ignore()).unwrap();

  // When: connecting the source outlet to the sink inlet
  let result = builder.connect(&source_outlet, &sink_inlet);

  // Then: the connection succeeds
  assert!(result.is_ok());
}

#[test]
fn create_flow_builds_flow_via_builder_block() {
  // Given: a create_flow call that adds a map flow and wires it
  let flow = GraphDsl::create_flow(|builder: &mut PublicGraphDslBuilder<u32, u32, StreamNotUsed>| {
    let (map_in, map_out) = builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|v: u32| v * 3)).unwrap();
    // The builder's main inlet connects to map_in, map_out connects to main outlet
    // (exact wiring depends on implementation; this tests the builder block pattern itself)
    let _ = (map_in, map_out);
  });

  // When: the flow is built successfully
  let (_graph, _mat) = flow.into_parts();

  // Then: the builder block executed without panic and produced a valid flow
}

#[test]
fn create_flow_mat_preserves_materialized_value() {
  // Given: a create_flow_mat call with an initial Mat value
  let flow = GraphDsl::create_flow_mat(99_u32, |_builder: &mut PublicGraphDslBuilder<u32, u32, u32>| {
    // No additional wiring needed for Mat-only test
  });

  // When: extracting the materialized value
  let (_graph, mat) = flow.into_parts();

  // Then: the initial Mat value is preserved
  assert_eq!(mat, 99_u32);
}

#[test]
fn add_source_and_connect_to_sink_produces_data() {
  // Given: a builder with explicitly added source, map flow, and sink
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::from_array([1_u32, 2, 3])).unwrap();
  let (map_in, map_out) = builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 10)).unwrap();
  let sink_in = builder.add_sink(Sink::<u32, _>::ignore()).unwrap();

  // When: wiring source → map → sink
  builder.connect(&source_out, &map_in).expect("connect source to map");
  builder.connect(&map_out, &sink_in).expect("connect map to sink");

  // Then: the graph can be built without error
  let _flow = builder.build();
}

#[test]
fn create_flow_empty_block_produces_valid_flow() {
  // Given: create_flow with an empty build block
  let flow = GraphDsl::create_flow(|_builder: &mut PublicGraphDslBuilder<u32, u32, StreamNotUsed>| {
    // empty — no additional wiring
  });

  // Then: the flow is successfully built without panic
  let (_graph, _mat) = flow.into_parts();
}

// ---------------------------------------------------------------------------
// Phase 3c: GraphDSL builder ergonomics — create_source / create_sink
// ---------------------------------------------------------------------------

#[test]
fn create_source_builds_source_via_builder_block() {
  // Given: a create_source call that adds a source and wires it
  let source = GraphDsl::create_source(|builder: &mut PublicGraphDslBuilder<(), u32, StreamNotUsed>| {
    let outlet = builder.add_source(Source::single(42_u32)).unwrap();
    let _ = outlet;
  });

  // When: the source is built successfully
  let (_graph, _mat) = source.into_parts();

  // Then: the builder block executed without panic and produced a valid source
}

#[test]
fn create_source_empty_block_produces_valid_source() {
  // Given: create_source with an empty build block
  let source = GraphDsl::create_source(|_builder: &mut PublicGraphDslBuilder<(), u32, StreamNotUsed>| {
    // empty — no additional wiring
  });

  // Then: the source is successfully built without panic
  let (_graph, _mat) = source.into_parts();
}

#[test]
fn create_sink_builds_sink_via_builder_block() {
  // Given: a create_sink call that adds a sink and wires it
  let sink = GraphDsl::create_sink(|builder: &mut PublicGraphDslBuilder<u32, (), StreamNotUsed>| {
    let inlet = builder.add_sink(Sink::<u32, _>::ignore()).unwrap();
    let _ = inlet;
  });

  // When: the sink is built successfully
  let (_graph, _mat) = sink.into_parts();

  // Then: the builder block executed without panic and produced a valid sink
}

#[test]
fn create_sink_empty_block_produces_valid_sink() {
  // Given: create_sink with an empty build block
  let sink = GraphDsl::create_sink(|_builder: &mut PublicGraphDslBuilder<u32, (), StreamNotUsed>| {
    // empty — no additional wiring
  });

  // Then: the sink is successfully built without panic
  let (_graph, _mat) = sink.into_parts();
}

// ---------------------------------------------------------------------------
// Phase 3c: GraphDSL builder ergonomics — add_*_mat (materialized value)
// ---------------------------------------------------------------------------

#[test]
fn add_source_mat_returns_outlet_and_materialized_value() {
  // Given: a source with a custom materialized value
  let source = Source::single(10_u32).map_materialized_value(|_| 77_i32);
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();

  // When: adding the source with mat
  let (outlet, mat) = builder.add_source_mat(source).unwrap();

  // Then: the outlet is valid and the materialized value is preserved
  let _ = outlet.id();
  assert_eq!(mat, 77_i32);
}

#[test]
fn add_flow_mat_returns_ports_and_materialized_value() {
  // Given: a flow with a custom materialized value
  let flow = Flow::<u32, u32, StreamNotUsed>::new().map(|v| v + 1).map_materialized_value(|_| 88_i32);
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();

  // When: adding the flow with mat
  let (inlet, outlet, mat) = builder.add_flow_mat(flow).unwrap();

  // Then: the ports are valid and the materialized value is preserved
  let _ = inlet.id();
  let _ = outlet.id();
  assert_eq!(mat, 88_i32);
}

#[test]
fn add_sink_mat_returns_inlet_and_materialized_value() {
  // Given: a sink with a custom materialized value
  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 99_i32);
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();

  // When: adding the sink with mat
  let (inlet, mat) = builder.add_sink_mat(sink).unwrap();

  // Then: the inlet is valid and the materialized value is preserved
  let _ = inlet.id();
  assert_eq!(mat, 99_i32);
}

// ---------------------------------------------------------------------------
// Phase 3c: GraphDSL builder ergonomics — connect_via
// ---------------------------------------------------------------------------

#[test]
fn connect_via_wires_outlet_through_flow_to_inlet() {
  // Given: a builder with a source and a sink added
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::single(5_u32)).unwrap();
  let sink_in = builder.add_sink(Sink::<u32, _>::ignore()).unwrap();
  let map_flow = Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 2);

  // When: connecting source → flow → sink in one step
  let result = builder.connect_via(&source_out, map_flow, &sink_in);

  // Then: the connection succeeds
  assert!(result.is_ok());
}

#[test]
fn connect_via_equivalent_to_manual_add_flow_and_connects() {
  // Given: a builder with a source and a sink
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::from_array([1_u32, 2, 3])).unwrap();
  let sink_in = builder.add_sink(Sink::<u32, _>::ignore()).unwrap();

  // When: using connect_via instead of manual add_flow + connect + connect
  let result = builder.connect_via(&source_out, Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 10), &sink_in);

  // Then: the graph can be built without error
  assert!(result.is_ok());
  let _flow = builder.build();
}

// --- wire_via ---

#[test]
fn wire_via_connects_outlet_through_flow_and_returns_downstream_outlet() {
  // Given: a builder with a source added
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::single(5_u32)).unwrap();

  // When: wiring through a mapping flow
  let map_flow = Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 2);
  let result = builder.wire_via(&source_out, map_flow);

  // Then: the result is Ok with a new outlet
  assert!(result.is_ok());
}

#[test]
fn wire_via_chained_produces_correct_result() {
  // Given: a source and two mapping flows wired in the builder
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::single(3_u32)).unwrap();

  // When: chaining two wire_via calls followed by add_sink_mat + connect
  let out1 = builder.wire_via(&source_out, Flow::<u32, u32, StreamNotUsed>::new().map(|v| v + 1)).unwrap();
  let out2 = builder.wire_via(&out1, Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 10)).unwrap();
  let (sink_in, completion) = builder.add_sink_mat(Sink::head()).unwrap();
  builder.connect(&out2, &sink_in).unwrap();

  // Then: running the graph directly produces (3+1)*10 = 40
  let (graph, _mat) = builder.into_parts();
  let plan = graph.into_plan().unwrap();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_terminal(&mut interpreter);
  assert_eq!(completion.value(), Completion::Ready(Ok(40_u32)));
}

// --- wire_to ---

#[test]
fn wire_to_connects_outlet_to_sink() {
  // Given: a builder with a source added
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::single(42_u32)).unwrap();

  // When: wiring directly to a sink
  let result = builder.wire_to(&source_out, Sink::<u32, _>::ignore());

  // Then: the connection succeeds
  assert!(result.is_ok());
}

#[test]
fn wire_to_equivalent_to_add_sink_plus_connect() {
  // Given: a builder with a source
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::from_array([1_u32, 2, 3])).unwrap();

  // When: using wire_to instead of manual add_sink + connect
  let result = builder.wire_to(&source_out, Sink::<u32, _>::ignore());

  // Then: the graph can be built
  assert!(result.is_ok());
  let _flow = builder.build();
}

// --- wire_from ---

#[test]
fn wire_from_connects_source_to_inlet() {
  // Given: a builder with a sink added (to get an inlet)
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let sink_in = builder.add_sink(Sink::<u32, _>::ignore()).unwrap();

  // When: wiring a source to the inlet
  let result = builder.wire_from(Source::single(7_u32), &sink_in);

  // Then: the connection succeeds
  assert!(result.is_ok());
}

#[test]
fn wire_from_equivalent_to_add_source_plus_connect() {
  // Given: a builder with a flow and sink added
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let (flow_in, flow_out) = builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|v| v + 1)).unwrap();
  let sink_in = builder.add_sink(Sink::<u32, _>::ignore()).unwrap();
  builder.connect(&flow_out, &sink_in).unwrap();

  // When: using wire_from to connect a source to the flow's inlet
  let result = builder.wire_from(Source::single(10_u32), &flow_in);

  // Then: the graph can be built
  assert!(result.is_ok());
  let _flow = builder.build();
}

// --- wire method integration: full linear chain ---

#[test]
fn wire_methods_build_full_linear_chain() {
  // Given: a source, two flows, and a sink
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();

  // When: building a linear chain with wire methods
  let out = builder.add_source(Source::single(2_u32)).unwrap();
  let out = builder.wire_via(&out, Flow::<u32, u32, StreamNotUsed>::new().map(|v| v + 3)).unwrap();
  let out = builder.wire_via(&out, Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 4)).unwrap();
  builder.wire_to(&out, Sink::<u32, _>::ignore()).unwrap();

  // Then: the graph can be built without error
  let _flow = builder.build();
}
