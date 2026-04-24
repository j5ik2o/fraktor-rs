use crate::core::{
  dsl::{Flow, Sink, Source},
  r#impl::{
    fusing::StreamBufferConfig, graph_dsl_builder::GraphDslBuilder, interpreter::graph_interpreter::GraphInterpreter,
    materialization::StreamState, port_ops::PortOps,
  },
  materialization::{Completion, DriveOutcome, StreamNotUsed},
  shape::Outlet,
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

// --- PortOps construction ---

#[test]
fn port_ops_new_wraps_outlet() {
  // Given: an outlet
  let outlet = Outlet::<u32>::new();

  // When: wrapping it in PortOps
  let ops = PortOps::new(&outlet);

  // Then: the outlet can be retrieved
  assert_eq!(ops.outlet(), outlet);
}

#[test]
fn port_ops_from_outlet_conversion() {
  // Given: an outlet
  let outlet = Outlet::<u32>::new();

  // When: converting via From trait
  let ops: PortOps<u32> = PortOps::from(outlet);

  // Then: the outlet matches
  assert_eq!(ops.outlet(), outlet);
}

#[test]
fn port_ops_is_copy() {
  // Given: a PortOps
  let outlet = Outlet::<u32>::new();
  let ops = PortOps::new(&outlet);

  // When: copying it
  let ops2 = ops;
  let _ops3 = ops; // must compile if Copy

  // Then: both copies are valid
  assert_eq!(ops2.outlet(), ops.outlet());
}

// --- PortOps::via ---

#[test]
fn port_ops_via_chains_through_flow() {
  // Given: a builder with a source
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::single(5_u32)).unwrap();

  // When: using PortOps::via to chain through a flow
  let result = PortOps::new(&source_out).via(Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 2), &mut builder);

  // Then: the result is Ok with a new PortOps
  assert!(result.is_ok());
}

#[test]
fn port_ops_via_chained_produces_correct_result() {
  // Given: a source with value 3 wired in the builder
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::single(3_u32)).unwrap();

  // When: chaining via + via, then add_sink_mat + connect
  let last = PortOps::new(&source_out)
    .via(Flow::<u32, u32, StreamNotUsed>::new().map(|v| v + 1), &mut builder)
    .unwrap()
    .via(Flow::<u32, u32, StreamNotUsed>::new().map(|v| v * 10), &mut builder)
    .unwrap();
  let (sink_in, completion) = builder.add_sink_mat(Sink::head()).unwrap();
  builder.connect(&last.outlet(), &sink_in).unwrap();

  // Then: running the graph directly produces (3+1)*10 = 40
  let (graph, _mat) = builder.into_parts();
  let plan = graph.into_plan().unwrap();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_terminal(&mut interpreter);
  assert_eq!(completion.poll(), Completion::Ready(Ok(40_u32)));
}

// --- PortOps::to ---

#[test]
fn port_ops_to_connects_to_sink() {
  // Given: a builder with a source
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::single(42_u32)).unwrap();

  // When: using PortOps::to to connect to a sink
  let result = PortOps::new(&source_out).to(Sink::<u32, _>::ignore(), &mut builder);

  // Then: the connection succeeds
  assert!(result.is_ok());
}

// --- PortOps::connect_to ---

#[test]
fn port_ops_connect_to_connects_to_inlet() {
  // Given: a builder with a source and a sink
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::single(7_u32)).unwrap();
  let sink_in = builder.add_sink(Sink::<u32, _>::ignore()).unwrap();

  // When: using PortOps::connect_to
  let result = PortOps::new(&source_out).connect_to(&sink_in, &mut builder);

  // Then: the connection succeeds
  assert!(result.is_ok());
}

// --- PortOps with fan-out pattern ---

#[test]
fn port_ops_supports_fan_out_via_outlet_copy() {
  // Given: a builder with a source outlet (Copy allows reuse)
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::single(5_u32)).unwrap();

  // When: creating PortOps from the outlet (PortOps is Copy and reusable)
  let ops = PortOps::new(&source_out);

  // Then: the outlet can be used in multiple contexts
  let _outlet = ops.outlet();
  let _outlet2 = ops.outlet();
  assert_eq!(_outlet, _outlet2);
}
