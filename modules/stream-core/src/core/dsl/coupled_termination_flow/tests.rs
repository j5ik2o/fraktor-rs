use alloc::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::core::{
  dsl::{CoupledTerminationFlow, Flow, Sink, Source},
  materialization::{Completion, KeepBoth, KeepLeft, KeepRight, StreamCompletion, StreamNotUsed},
};

// ---------------------------------------------------------------------------
// Batch 8 Task W: CoupledTerminationFlow factory struct
// ---------------------------------------------------------------------------

#[test]
fn coupled_termination_flow_from_sink_and_source_returns_flow_with_stream_not_used_mat() {
  // Given: a trivial sink and a single-element source
  let sink = Sink::<u32, _>::ignore();
  let source = Source::single(99_u32);

  // When: constructing a coupled-termination flow through the public factory
  let flow: Flow<u32, u32, StreamNotUsed> = CoupledTerminationFlow::from_sink_and_source(sink, source);

  // Then: the materialized value is StreamNotUsed (Pekko NotUsed 等価)
  let (_graph, mat) = flow.into_parts();
  assert_eq!(mat, StreamNotUsed::new());
}

#[test]
fn coupled_termination_flow_from_sink_and_source_emits_elements_from_embedded_source() {
  // Given: an embedded source that emits 42; upstream elements are consumed by the sink
  let sink = Sink::<u32, _>::ignore();
  let source = Source::single(42_u32);
  let flow = CoupledTerminationFlow::from_sink_and_source(sink, source);

  // When: the coupled flow is driven by an unrelated upstream value
  let values = Source::single(7_u32).via(flow).collect_values().expect("collect_values");

  // Then: the output comes from the embedded source, not the upstream element
  assert_eq!(values, alloc::vec![42_u32]);
}

#[test]
fn coupled_termination_flow_from_sink_and_source_mat_keep_left_keeps_sink_materialized_value() {
  // Given: a sink carrying 99_i32 and a source carrying true
  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 99_i32);
  let source = Source::single(1_u32).map_materialized_value(|_| true);

  // When: KeepLeft rule is used for materialization combination
  let flow = CoupledTerminationFlow::from_sink_and_source_mat(sink, source, KeepLeft);

  // Then: only the sink's materialized value survives
  let (_graph, mat) = flow.into_parts();
  assert_eq!(mat, 99_i32);
}

#[test]
fn coupled_termination_flow_from_sink_and_source_mat_keep_right_keeps_source_materialized_value() {
  // Given: a sink carrying 99_i32 and a source carrying true
  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 99_i32);
  let source = Source::single(1_u32).map_materialized_value(|_| true);

  // When: KeepRight rule is used
  let flow = CoupledTerminationFlow::from_sink_and_source_mat(sink, source, KeepRight);

  // Then: only the source's materialized value survives
  let (_graph, mat) = flow.into_parts();
  assert!(mat);
}

#[test]
fn coupled_termination_flow_from_sink_and_source_mat_keep_both_keeps_both_materialized_values() {
  // Given: distinct materialized values on sink and source
  let sink = Sink::<u32, _>::ignore().map_materialized_value(|_| 99_i32);
  let source = Source::single(1_u32).map_materialized_value(|_| true);

  // When: KeepBoth rule is used
  let flow = CoupledTerminationFlow::from_sink_and_source_mat(sink, source, KeepBoth);

  // Then: the materialized tuple carries both sides
  let (_graph, (left, right)) = flow.into_parts();
  assert_eq!(left, 99_i32);
  assert!(right);
}

#[test]
fn coupled_termination_flow_completes_wrapped_sink_when_embedded_source_finishes() {
  // Given: a sink that tracks completion and an empty source that finishes immediately
  let sink_completed = Arc::new(AtomicBool::new(false));
  let sink = Sink::<u32, _>::on_complete({
    let sink_completed = sink_completed.clone();
    move |_| sink_completed.store(true, Ordering::SeqCst)
  });
  let source = Source::<u32, _>::empty().watch_termination_mat(KeepRight);

  // When: the coupled flow is driven through an upstream single-element source
  let flow = CoupledTerminationFlow::from_sink_and_source_mat(sink, source, KeepRight);
  let (graph, right_completion) = flow.into_parts();
  let source_flow: Flow<u32, u32, StreamCompletion<()>> = Flow::from_graph(graph, right_completion.clone());
  let values = Source::single(1_u32).via(source_flow).collect_values().expect("collect_values");

  // Then: the embedded source's completion propagates to the sink (coupled termination)
  assert!(values.is_empty());
  assert!(sink_completed.load(Ordering::SeqCst));
  assert_eq!(right_completion.poll(), Completion::Ready(Ok(())));
}

#[test]
fn coupled_termination_flow_cancels_embedded_source_when_wrapped_sink_cancels() {
  // Given: a never-completing source with watch_termination and a sink that cancels immediately
  let source = Source::<u32, _>::never().watch_termination_mat(KeepRight);
  let sink = Sink::<u32, _>::cancelled();

  // When: the coupled flow is driven
  let flow = CoupledTerminationFlow::from_sink_and_source_mat(sink, source, KeepRight);
  let (graph, right_completion) = flow.into_parts();
  let source_flow: Flow<u32, u32, StreamCompletion<()>> = Flow::from_graph(graph, right_completion.clone());
  let values = Source::single(1_u32).via(source_flow).collect_values().expect("collect_values");

  // Then: the sink's cancellation stops the embedded source (bidirectional coupling)
  assert!(values.is_empty());
  assert_eq!(right_completion.poll(), Completion::Ready(Ok(())));
}

#[test]
fn coupled_termination_flow_from_sink_and_source_is_equivalent_to_flow_from_sink_and_source_coupled() {
  // Given: identical sink / source arguments passed to both entry points
  let sink_a = Sink::<u32, _>::ignore();
  let source_a = Source::single(123_u32);
  let sink_b = Sink::<u32, _>::ignore();
  let source_b = Source::single(123_u32);

  // When: one flow is built through the public factory and the other through
  // Flow::from_sink_and_source_coupled
  let via_factory: Flow<u32, u32, StreamNotUsed> = CoupledTerminationFlow::from_sink_and_source(sink_a, source_a);
  let via_flow: Flow<u32, u32, StreamNotUsed> = Flow::from_sink_and_source_coupled(sink_b, source_b);

  // Then: both yield the same runtime behaviour (delegated construction equivalence)
  let values_factory = Source::single(0_u32).via(via_factory).collect_values().expect("collect_values");
  let values_flow = Source::single(0_u32).via(via_flow).collect_values().expect("collect_values");

  assert_eq!(values_factory, values_flow);
  assert_eq!(values_factory, alloc::vec![123_u32]);
}

#[test]
fn coupled_termination_flow_from_sink_and_source_mat_is_equivalent_to_flow_from_sink_and_source_coupled_mat() {
  // Given: identical arguments and KeepLeft combination rule for both entry points
  let sink_a = Sink::<u32, _>::ignore().map_materialized_value(|_| 3_u32);
  let source_a = Source::single(99_u32).map_materialized_value(|_| 4_u32);
  let sink_b = Sink::<u32, _>::ignore().map_materialized_value(|_| 3_u32);
  let source_b = Source::single(99_u32).map_materialized_value(|_| 4_u32);

  // When: constructing through both entry points with KeepLeft
  let via_factory = CoupledTerminationFlow::from_sink_and_source_mat(sink_a, source_a, KeepLeft);
  let via_flow = Flow::<u32, u32, StreamNotUsed>::from_sink_and_source_coupled_mat(sink_b, source_b, KeepLeft);

  // Then: both materialized values equal 3_u32 (sink side kept)
  let (_g1, mat_factory) = via_factory.into_parts();
  let (_g2, mat_flow) = via_flow.into_parts();
  assert_eq!(mat_factory, 3_u32);
  assert_eq!(mat_flow, 3_u32);
  assert_eq!(mat_factory, mat_flow);
}
