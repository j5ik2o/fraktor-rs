use crate::core::{
  StreamError, StreamNotUsed,
  graph::{GraphDslBuilder, ReversePortOps},
  shape::Inlet,
  stage::{Sink, Source, flow::Flow},
};

// --- ReversePortOps construction ---

#[test]
fn reverse_port_ops_new_wraps_inlet() {
  // Given: an inlet
  let inlet = Inlet::<u32>::new();

  // When: wrapping it in ReversePortOps
  let ops = ReversePortOps::new(&inlet);

  // Then: the inlet can be retrieved
  assert_eq!(ops.inlet(), inlet);
}

#[test]
fn reverse_port_ops_from_inlet_conversion() {
  // Given: an inlet
  let inlet = Inlet::<u32>::new();

  // When: converting via From trait
  let ops: ReversePortOps<u32> = ReversePortOps::from(inlet);

  // Then: the inlet matches
  assert_eq!(ops.inlet(), inlet);
}

#[test]
fn reverse_port_ops_is_copy() {
  // Given: a ReversePortOps
  let inlet = Inlet::<u32>::new();
  let ops = ReversePortOps::new(&inlet);

  // When: copying it
  let ops2 = ops;
  let _ops3 = ops; // must compile if Copy

  // Then: both copies are valid
  assert_eq!(ops2.inlet(), ops.inlet());
}

// --- ReversePortOps::from_source ---

#[test]
fn reverse_port_ops_from_source_connects_source_to_inlet() -> Result<(), StreamError> {
  // Given: a builder with a sink added (to get an inlet)
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let sink_in = builder.add_sink(Sink::<u32, _>::ignore())?;

  // When: using ReversePortOps to connect a source to the inlet
  let result = ReversePortOps::new(&sink_in).from_source(Source::single(7_u32), &mut builder);

  // Then: the connection succeeds
  assert!(result.is_ok());
  Ok(())
}

#[test]
fn reverse_port_ops_from_source_with_flow_inlet() -> Result<(), StreamError> {
  // Given: a builder with a flow and sink connected
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let (flow_in, flow_out) = builder.add_flow(Flow::<u32, u32, StreamNotUsed>::new().map(|v| v + 1))?;
  let sink_in = builder.add_sink(Sink::<u32, _>::ignore())?;
  builder.connect(&flow_out, &sink_in)?;

  // When: connecting a source to the flow's inlet via ReversePortOps
  let result = ReversePortOps::new(&flow_in).from_source(Source::single(10_u32), &mut builder);

  // Then: the connection succeeds and graph can be built
  assert!(result.is_ok());
  assert!(builder.build().into_parts().0.into_plan().is_ok());
  Ok(())
}

// --- ReversePortOps::connect_from ---

#[test]
fn reverse_port_ops_connect_from_connects_outlet_to_inlet() -> Result<(), StreamError> {
  // Given: a builder with a source and a sink
  let mut builder = GraphDslBuilder::<u32, u32, StreamNotUsed>::new();
  let source_out = builder.add_source(Source::single(42_u32))?;
  let sink_in = builder.add_sink(Sink::<u32, _>::ignore())?;

  // When: using ReversePortOps::connect_from
  let result = ReversePortOps::new(&sink_in).connect_from(&source_out, &mut builder);

  // Then: the connection succeeds
  assert!(result.is_ok());
  Ok(())
}
