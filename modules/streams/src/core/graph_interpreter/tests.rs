use core::any::TypeId;
use std::panic::{AssertUnwindSafe, catch_unwind};

use fraktor_utils_rs::core::{
  collections::queue::OverflowPolicy,
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

use super::super::flow::{
  async_boundary_definition, balance_definition, broadcast_definition, buffer_definition, concat_definition,
  flat_map_merge_definition, merge_definition, merge_substreams_with_parallelism_definition, split_after_definition,
  split_when_definition, zip_definition,
};
use crate::core::{
  Completion, DemandTracker, DriveOutcome, DynValue, Flow, FlowDefinition, FlowLogic, GraphInterpreter, Inlet,
  KeepRight, MatCombine, Outlet, RestartBackoff, Sink, SinkDecision, SinkDefinition, SinkLogic, Source,
  SourceDefinition, SourceLogic, StageDefinition, StageKind, StreamBufferConfig, StreamCompletion, StreamDone,
  StreamError, StreamNotUsed, StreamPlan, StreamState, SupervisionStrategy,
};

fn drive_to_completion(interpreter: &mut GraphInterpreter) {
  interpreter.start().expect("start");
  while interpreter.state() == StreamState::Running {
    let _ = interpreter.drive();
  }
}

fn linear_plan(source: SourceDefinition, flows: Vec<FlowDefinition>, sink: SinkDefinition) -> StreamPlan {
  let mut stages = Vec::new();
  let mut edges = Vec::new();
  let mut upstream_outlet = source.outlet;
  stages.push(StageDefinition::Source(source));
  for flow in flows {
    edges.push((upstream_outlet, flow.inlet, flow.mat_combine));
    upstream_outlet = flow.outlet;
    stages.push(StageDefinition::Flow(flow));
  }
  edges.push((upstream_outlet, sink.inlet, sink.mat_combine));
  stages.push(StageDefinition::Sink(sink));
  StreamPlan::from_parts(stages, edges)
}

#[test]
fn source_map_fold_completes() {
  let graph =
    Source::single(1_u32).map(|value| value + 1).to_mat(Sink::fold(0_u32, |acc, value| acc + value), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(2)));
}

#[test]
fn source_head_completes_after_first() {
  let graph = Source::single(5_u32).to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(5)));
}

#[test]
fn take_until_requests_source_shutdown_after_first_match() {
  let pulls = ArcShared::new(SpinSyncMutex::new(0_u32));
  let cancels = ArcShared::new(SpinSyncMutex::new(0_u32));
  let graph = Source::<u32, _>::from_logic(StageKind::Custom, CancelAwareSequenceSourceLogic {
    next:    1,
    end:     100,
    pulls:   pulls.clone(),
    cancels: cancels.clone(),
  })
  .take_until(|value| *value >= 3)
  .to_mat(
    Sink::fold(Vec::new(), |mut acc: Vec<u32>, value| {
      acc.push(value);
      acc
    }),
    KeepRight,
  );
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![1_u32, 2_u32, 3_u32])));
  assert_eq!(*cancels.lock(), 1_u32);
  assert!(*pulls.lock() < 100_u32);
}

#[test]
fn flat_map_concat_uses_inner_source() {
  let graph = Source::single(1_u32).flat_map_concat(|value| Source::single(value + 1)).to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(completion.poll(), Completion::Ready(Ok(2)));
}

#[test]
fn flat_map_concat_respects_backpressure_when_inner_emits_multiple_elements() {
  let graph = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic { next: 1, end: 2 })
    .flat_map_concat(|value| Source::single(value).broadcast(2))
    .to_mat(
      Sink::fold(Vec::new(), |mut acc: Vec<u32>, value| {
        acc.push(value);
        acc
      }),
      KeepRight,
    );
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::new(1, OverflowPolicy::Block));
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![1_u32, 1_u32, 2_u32, 2_u32])));
}

#[test]
fn flat_map_merge_uses_configured_breadth() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 3 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let flat_map_merge =
    flat_map_merge_definition::<u32, u32, StreamNotUsed, _>(2, |value| Source::single(value).broadcast(2));
  let flat_map_merge_inlet = flat_map_merge.inlet;
  let flat_map_merge_outlet = flat_map_merge.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(CollectSequenceSinkLogic { completion: completion.clone(), values: Vec::new() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(flat_map_merge), StageDefinition::Sink(sink)],
    vec![
      (source_outlet.id(), flat_map_merge_inlet, MatCombine::KeepLeft),
      (flat_map_merge_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![1, 1, 2, 3, 2, 3])));
}

#[test]
fn flat_map_merge_delays_new_inner_creation_until_breadth_slot_is_released() {
  let created = ArcShared::new(SpinSyncMutex::new(0_u32));
  let graph = Source::<u32, _>::from_logic(StageKind::Custom, SequenceSourceLogic { next: 1, end: 2 })
    .flat_map_merge(1, {
      let created = created.clone();
      move |value| {
        let mut guard = created.lock();
        *guard = guard.saturating_add(1);
        Source::single(value).broadcast(2)
      }
    })
    .expect("flat_map_merge")
    .to_mat(Sink::ignore(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  interpreter.start().expect("start");

  assert_eq!(*created.lock(), 0);
  assert_eq!(interpreter.drive(), DriveOutcome::Progressed);
  assert_eq!(*created.lock(), 1);
  assert_eq!(interpreter.drive(), DriveOutcome::Progressed);
  assert_eq!(*created.lock(), 1);

  while interpreter.state() == StreamState::Running {
    let _ = interpreter.drive();
  }

  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(*created.lock(), 2);
  assert_eq!(completion.poll(), Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn flat_map_concat_fails_stream_when_inner_source_fails_without_recovery() {
  struct FailingInnerSourceLogic;

  impl SourceLogic for FailingInnerSourceLogic {
    fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
      Err(StreamError::Failed)
    }
  }

  let graph = Source::single(1_u32)
    .flat_map_concat(|_| Source::<u32, _>::from_logic(StageKind::Custom, FailingInnerSourceLogic))
    .to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Failed);
  assert_eq!(completion.poll(), Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn flat_map_merge_fails_stream_when_inner_source_fails_without_recovery() {
  struct FailingInnerSourceLogic;

  impl SourceLogic for FailingInnerSourceLogic {
    fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
      Err(StreamError::Failed)
    }
  }

  let graph = Source::single(1_u32)
    .flat_map_merge(1, |_| Source::<u32, _>::from_logic(StageKind::Custom, FailingInnerSourceLogic))
    .expect("flat_map_merge")
    .to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Failed);
  assert_eq!(completion.poll(), Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn buffer_flow_fails_with_block_policy_on_overflow() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 3 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let buffer = buffer_definition::<u32>(2, OverflowPolicy::Block);
  let buffer_inlet = buffer.inlet;
  let buffer_outlet = buffer.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion: completion.clone() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(buffer), StageDefinition::Sink(sink)],
    vec![
      (source_outlet.id(), buffer_inlet, MatCombine::KeepLeft),
      (buffer_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Failed);
  assert_eq!(completion.poll(), Completion::Ready(Err(StreamError::BufferOverflow)));
}

#[test]
fn buffer_flow_drop_oldest_keeps_latest_elements() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 3 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let buffer = buffer_definition::<u32>(2, OverflowPolicy::DropOldest);
  let buffer_inlet = buffer.inlet;
  let buffer_outlet = buffer.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(CollectSequenceSinkLogic { completion: completion.clone(), values: Vec::new() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(buffer), StageDefinition::Sink(sink)],
    vec![
      (source_outlet.id(), buffer_inlet, MatCombine::KeepLeft),
      (buffer_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![2, 3])));
}

#[test]
fn async_boundary_flow_preserves_input_order() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 3 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let async_boundary = async_boundary_definition::<u32>();
  let async_boundary_inlet = async_boundary.inlet;
  let async_boundary_outlet = async_boundary.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(CollectSequenceSinkLogic { completion: completion.clone(), values: Vec::new() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(async_boundary), StageDefinition::Sink(sink)],
    vec![
      (source_outlet.id(), async_boundary_inlet, MatCombine::KeepLeft),
      (async_boundary_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![1, 2, 3])));
}

#[test]
fn group_by_uses_key_function() {
  let graph = Source::single(3_u32)
    .group_by(4, |value: &u32| value % 2)
    .expect("group_by")
    .merge_substreams()
    .to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(3_u32)));
}

#[test]
fn recover_flow_replaces_error_payload() {
  let graph =
    Source::single(Err::<u32, StreamError>(StreamError::Failed)).recover(10_u32).to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(10_u32)));
}

#[test]
fn recover_with_retries_flow_fails_when_retry_budget_is_zero() {
  let graph = Source::single(Err::<u32, StreamError>(StreamError::Failed))
    .recover_with_retries(0, 10_u32)
    .to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Failed);
  assert_eq!(completion.poll(), Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn restart_sink_with_backoff_keeps_single_path_behavior() {
  let graph = Source::single(5_u32).to_mat(Sink::head().restart_sink_with_backoff(1, 3), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(5_u32)));
}

#[test]
fn sink_supervision_variants_keep_single_path_behavior() {
  let graph =
    Source::single(5_u32).to_mat(Sink::head().supervision_stop().supervision_resume().supervision_restart(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(5_u32)));
}

#[test]
fn restart_budget_exhaustion_completes_with_default_terminal_action() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();
  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(AlwaysFailSourceLogic),
    supervision: SupervisionStrategy::Stop,
    restart:     Some(RestartBackoff::new(0, 1)),
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion: completion.clone() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let plan = StreamPlan::from_parts(vec![StageDefinition::Source(source), StageDefinition::Sink(sink)], vec![(
    source_outlet.id(),
    sink_inlet.id(),
    MatCombine::KeepRight,
  )]);
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn source_completion_triggers_restart_until_budget_is_exhausted() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();
  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RestartableSingleSourceLogic { value: 9, emitted: false }),
    supervision: SupervisionStrategy::Stop,
    restart:     Some(RestartBackoff::new(0, 1)),
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(CollectSequenceSinkLogic { completion: completion.clone(), values: Vec::new() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let plan = StreamPlan::from_parts(vec![StageDefinition::Source(source), StageDefinition::Sink(sink)], vec![(
    source_outlet.id(),
    sink_inlet.id(),
    MatCombine::KeepRight,
  )]);
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![9_u32, 9_u32])));
}

#[test]
fn split_when_restart_supervision_behaves_like_resume() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let flow_inlet: Inlet<u32> = Inlet::new();
  let flow_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();
  let restart_calls = ArcShared::new(SpinSyncMutex::new(0_u32));

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 3 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let flow = FlowDefinition {
    kind:        StageKind::FlowSplitWhen,
    inlet:       flow_inlet.id(),
    outlet:      flow_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(RestartCounterFlowLogic { restart_calls: restart_calls.clone() }),
    supervision: SupervisionStrategy::Restart,
    restart:     None,
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion: completion.clone() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let plan = linear_plan(source, vec![flow], sink);
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(StreamDone::new())));
  assert_eq!(*restart_calls.lock(), 0_u32);
}

#[test]
fn non_split_restart_supervision_calls_on_restart() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let flow_inlet: Inlet<u32> = Inlet::new();
  let flow_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();
  let restart_calls = ArcShared::new(SpinSyncMutex::new(0_u32));

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 3 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       flow_inlet.id(),
    outlet:      flow_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(RestartCounterFlowLogic { restart_calls: restart_calls.clone() }),
    supervision: SupervisionStrategy::Restart,
    restart:     None,
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion: completion.clone() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let plan = linear_plan(source, vec![flow], sink);
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(StreamDone::new())));
  assert!(*restart_calls.lock() > 0_u32);
}

#[test]
fn async_boundary_backpressures_instead_of_failing_when_downstream_stalls() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let pulls = ArcShared::new(SpinSyncMutex::new(0_u32));
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(CountingSourceLogic { remaining: 8, pulls: pulls.clone() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let async_boundary = async_boundary_definition::<u32>();
  let async_boundary_inlet = async_boundary.inlet;
  let async_boundary_outlet = async_boundary.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(BlockedSinkLogic { completion }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(async_boundary), StageDefinition::Sink(sink)],
    vec![
      (source_outlet.id(), async_boundary_inlet, MatCombine::KeepLeft),
      (async_boundary_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::new(1, OverflowPolicy::Block));
  interpreter.start().expect("start");
  for _ in 0..8 {
    let _ = interpreter.drive();
    assert_ne!(interpreter.state(), StreamState::Failed);
  }
  assert_eq!(interpreter.state(), StreamState::Running);
  assert_eq!(*pulls.lock(), 2_u32);
}

#[test]
fn cross_operator_backpressure_propagates_through_substream_and_async_boundary() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let pulls = ArcShared::new(SpinSyncMutex::new(0_u32));
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(CountingSourceLogic { remaining: 12, pulls: pulls.clone() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let flat_map_merge =
    flat_map_merge_definition::<u32, u32, StreamNotUsed, _>(1, |value| Source::single(value).broadcast(2));
  let flat_map_merge_inlet = flat_map_merge.inlet;
  let flat_map_merge_outlet = flat_map_merge.outlet;
  let split_after = split_after_definition::<u32, _>(|_| true);
  let split_after_inlet = split_after.inlet;
  let split_after_outlet = split_after.outlet;
  let merge_substreams = merge_substreams_with_parallelism_definition::<u32>(1);
  let merge_substreams_inlet = merge_substreams.inlet;
  let merge_substreams_outlet = merge_substreams.outlet;
  let async_boundary = async_boundary_definition::<u32>();
  let async_boundary_inlet = async_boundary.inlet;
  let async_boundary_outlet = async_boundary.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(BlockedSinkLogic { completion }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let plan = StreamPlan::from_parts(
    vec![
      StageDefinition::Source(source),
      StageDefinition::Flow(flat_map_merge),
      StageDefinition::Flow(split_after),
      StageDefinition::Flow(merge_substreams),
      StageDefinition::Flow(async_boundary),
      StageDefinition::Sink(sink),
    ],
    vec![
      (source_outlet.id(), flat_map_merge_inlet, MatCombine::KeepLeft),
      (flat_map_merge_outlet, split_after_inlet, MatCombine::KeepLeft),
      (split_after_outlet, merge_substreams_inlet, MatCombine::KeepLeft),
      (merge_substreams_outlet, async_boundary_inlet, MatCombine::KeepLeft),
      (async_boundary_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::new(1, OverflowPolicy::Block));
  interpreter.start().expect("start");
  for _ in 0..16 {
    let _ = interpreter.drive();
    assert_ne!(interpreter.state(), StreamState::Failed);
  }
  assert_eq!(interpreter.state(), StreamState::Running);
  assert!(*pulls.lock() <= 3_u32);
}

#[test]
fn cross_operator_failure_propagates_from_flat_map_to_substream_merge_chain() {
  struct FailingInnerSourceLogic;

  impl SourceLogic for FailingInnerSourceLogic {
    fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
      Err(StreamError::Failed)
    }
  }

  let graph = Source::single(1_u32)
    .flat_map_merge(1, |_| Source::<u32, _>::from_logic(StageKind::Custom, FailingInnerSourceLogic))
    .expect("flat_map_merge")
    .split_after(|_| true)
    .merge_substreams_with_parallelism(1)
    .expect("merge_substreams_with_parallelism")
    .async_boundary()
    .to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::new(1, OverflowPolicy::Block));
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Failed);
  assert_eq!(completion.poll(), Completion::Ready(Err(StreamError::Failed)));
}

#[test]
fn source_restart_is_preserved_across_substream_and_async_boundary_chain() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RestartableSingleSourceLogic { value: 7, emitted: false }),
    supervision: SupervisionStrategy::Stop,
    restart:     Some(RestartBackoff::new(0, 1)),
  };
  let split_after = split_after_definition::<u32, _>(|_| true);
  let split_after_inlet = split_after.inlet;
  let split_after_outlet = split_after.outlet;
  let merge_substreams = merge_substreams_with_parallelism_definition::<u32>(1);
  let merge_substreams_inlet = merge_substreams.inlet;
  let merge_substreams_outlet = merge_substreams.outlet;
  let async_boundary = async_boundary_definition::<u32>();
  let async_boundary_inlet = async_boundary.inlet;
  let async_boundary_outlet = async_boundary.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(CollectSequenceSinkLogic { completion: completion.clone(), values: Vec::new() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let plan = StreamPlan::from_parts(
    vec![
      StageDefinition::Source(source),
      StageDefinition::Flow(split_after),
      StageDefinition::Flow(merge_substreams),
      StageDefinition::Flow(async_boundary),
      StageDefinition::Sink(sink),
    ],
    vec![
      (source_outlet.id(), split_after_inlet, MatCombine::KeepLeft),
      (split_after_outlet, merge_substreams_inlet, MatCombine::KeepLeft),
      (merge_substreams_outlet, async_boundary_inlet, MatCombine::KeepLeft),
      (async_boundary_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![7_u32, 7_u32])));
}

#[test]
fn split_when_flow_splits_before_predicate() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<Vec<u32>> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 4 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let split_when = split_when_definition::<u32, _>(|value| value % 2 == 0);
  let split_when_inlet = split_when.inlet;
  let split_when_outlet = split_when.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<Vec<u32>>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(CollectNestedSequenceSinkLogic { completion: completion.clone(), values: Vec::new() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(split_when), StageDefinition::Sink(sink)],
    vec![
      (source_outlet.id(), split_when_inlet, MatCombine::KeepLeft),
      (split_when_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![vec![1_u32], vec![2_u32, 3_u32], vec![4_u32]])));
}

#[test]
fn split_after_flow_splits_after_predicate() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<Vec<u32>> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 4 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let split_after = split_after_definition::<u32, _>(|value| value % 2 == 0);
  let split_after_inlet = split_after.inlet;
  let split_after_outlet = split_after.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<Vec<u32>>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(CollectNestedSequenceSinkLogic { completion: completion.clone(), values: Vec::new() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(split_after), StageDefinition::Sink(sink)],
    vec![
      (source_outlet.id(), split_after_inlet, MatCombine::KeepLeft),
      (split_after_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![vec![1_u32, 2_u32], vec![3_u32, 4_u32]])));
}

#[test]
fn merge_substreams_flattens_segment_elements() {
  let graph = Source::single(vec![1_u32, 2_u32, 3_u32]).via(Flow::new().merge_substreams()).to_mat(
    Sink::fold(Vec::<u32>::new(), |mut acc, value| {
      acc.push(value);
      acc
    }),
    KeepRight,
  );
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![1_u32, 2_u32, 3_u32])));
}

#[test]
fn concat_substreams_flattens_segment_elements() {
  let graph = Source::single(vec![1_u32, 2_u32, 3_u32]).via(Flow::new().concat_substreams()).to_mat(
    Sink::fold(Vec::<u32>::new(), |mut acc, value| {
      acc.push(value);
      acc
    }),
    KeepRight,
  );
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![1_u32, 2_u32, 3_u32])));
}

#[test]
fn cancel_updates_state() {
  let graph = Source::single(1_u32).to_mat(Sink::ignore(), KeepRight);
  let (plan, _completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  interpreter.start().expect("start");
  interpreter.cancel().expect("cancel");
  assert_eq!(interpreter.state(), StreamState::Cancelled);
}

#[test]
fn drive_does_not_pull_without_demand() {
  let pulls = ArcShared::new(SpinSyncMutex::new(0_u32));
  let outlet: Outlet<u32> = Outlet::new();
  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(CountingSourceLogic { remaining: 1, pulls: pulls.clone() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(NoDemandSinkLogic { completion }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let plan = linear_plan(source, Vec::new(), sink);
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  interpreter.start().expect("start");
  let outcome = interpreter.drive();
  assert_eq!(outcome, DriveOutcome::Idle);
  assert_eq!(*pulls.lock(), 0);
}

#[test]
fn drive_rejects_type_mismatch() {
  let completion = StreamCompletion::new();
  let outlet: Outlet<u32> = Outlet::new();
  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SingleValueSourceLogic { value: Some(1_u32) }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let inlet: Inlet<u32> = Inlet::new();
  let flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       inlet.id(),
    outlet:      Outlet::<u32>::new().id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(MismatchFlowLogic),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let sink_inlet: Inlet<u32> = Inlet::new();
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion: completion.clone() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let plan = linear_plan(source, vec![flow], sink);
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  interpreter.start().expect("start");
  while interpreter.state() == StreamState::Running {
    let _ = interpreter.drive();
  }
  assert_eq!(interpreter.state(), StreamState::Failed);
  assert_eq!(completion.poll(), Completion::Ready(Err(StreamError::TypeMismatch)));
}

#[test]
fn executes_with_topologically_sorted_flow_order() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let flow1_inlet: Inlet<u32> = Inlet::new();
  let flow1_outlet: Outlet<u32> = Outlet::new();
  let flow2_inlet: Inlet<u32> = Inlet::new();
  let flow2_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SingleValueSourceLogic { value: Some(1_u32) }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let flow1 = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       flow1_inlet.id(),
    outlet:      flow1_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(IncrementFlowLogic),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let flow2 = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       flow2_inlet.id(),
    outlet:      flow2_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(IncrementFlowLogic),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion: completion.clone() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![
      StageDefinition::Source(source),
      StageDefinition::Flow(flow2),
      StageDefinition::Sink(sink),
      StageDefinition::Flow(flow1),
    ],
    vec![
      (source_outlet.id(), flow1_inlet.id(), MatCombine::KeepLeft),
      (flow1_outlet.id(), flow2_inlet.id(), MatCombine::KeepLeft),
      (flow2_outlet.id(), sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(StreamDone::new())));
}

#[test]
fn rejects_cycle_plan_on_construction() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let flow_inlet: Inlet<u32> = Inlet::new();
  let flow_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SingleValueSourceLogic { value: Some(1_u32) }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       flow_inlet.id(),
    outlet:      flow_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(IncrementFlowLogic),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(flow), StageDefinition::Sink(sink)],
    vec![
      (source_outlet.id(), flow_inlet.id(), MatCombine::KeepLeft),
      (flow_outlet.id(), flow_inlet.id(), MatCombine::KeepLeft),
      (flow_outlet.id(), sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let result = catch_unwind(AssertUnwindSafe(|| {
    let _ = GraphInterpreter::new(plan, StreamBufferConfig::default());
  }));
  assert!(result.is_err());
}

#[test]
fn supports_multiple_outgoing_edges_from_source() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let left_inlet: Inlet<u32> = Inlet::new();
  let left_outlet: Outlet<u32> = Outlet::new();
  let right_inlet: Inlet<u32> = Inlet::new();
  let right_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 4 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let left_flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       left_inlet.id(),
    outlet:      left_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(AddFlowLogic { add: 10 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let right_flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       right_inlet.id(),
    outlet:      right_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(AddFlowLogic { add: 100 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SumSinkLogic { completion: completion.clone(), sum: 0 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![
      StageDefinition::Source(source),
      StageDefinition::Flow(left_flow),
      StageDefinition::Flow(right_flow),
      StageDefinition::Sink(sink),
    ],
    vec![
      (source_outlet.id(), left_inlet.id(), MatCombine::KeepLeft),
      (source_outlet.id(), right_inlet.id(), MatCombine::KeepLeft),
      (left_outlet.id(), sink_inlet.id(), MatCombine::KeepRight),
      (right_outlet.id(), sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(230)));
}

#[test]
fn supports_multiple_outgoing_edges_from_flow() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let split_inlet: Inlet<u32> = Inlet::new();
  let split_outlet: Outlet<u32> = Outlet::new();
  let right_inlet: Inlet<u32> = Inlet::new();
  let right_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 4 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let split_flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       split_inlet.id(),
    outlet:      split_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(AddFlowLogic { add: 0 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let right_flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       right_inlet.id(),
    outlet:      right_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(AddFlowLogic { add: 100 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SumSinkLogic { completion: completion.clone(), sum: 0 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![
      StageDefinition::Source(source),
      StageDefinition::Flow(split_flow),
      StageDefinition::Flow(right_flow),
      StageDefinition::Sink(sink),
    ],
    vec![
      (source_outlet.id(), split_inlet.id(), MatCombine::KeepLeft),
      (split_outlet.id(), sink_inlet.id(), MatCombine::KeepRight),
      (split_outlet.id(), right_inlet.id(), MatCombine::KeepLeft),
      (right_outlet.id(), sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(210)));
}

#[test]
fn broadcast_flow_duplicates_elements_to_all_outgoing_edges() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let right_inlet: Inlet<u32> = Inlet::new();
  let right_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 3 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let broadcast = broadcast_definition::<u32>(2);
  let broadcast_inlet = broadcast.inlet;
  let broadcast_outlet = broadcast.outlet;
  let right_flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       right_inlet.id(),
    outlet:      right_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(AddFlowLogic { add: 100 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SumSinkLogic { completion: completion.clone(), sum: 0 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![
      StageDefinition::Source(source),
      StageDefinition::Flow(broadcast),
      StageDefinition::Flow(right_flow),
      StageDefinition::Sink(sink),
    ],
    vec![
      (source_outlet.id(), broadcast_inlet, MatCombine::KeepLeft),
      (broadcast_outlet, sink_inlet.id(), MatCombine::KeepRight),
      (broadcast_outlet, right_inlet.id(), MatCombine::KeepLeft),
      (right_outlet.id(), sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(312)));
}

#[test]
fn rejects_broadcast_flow_when_fan_out_does_not_match_wiring() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SingleValueSourceLogic { value: Some(1_u32) }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let broadcast = broadcast_definition::<u32>(2);
  let broadcast_inlet = broadcast.inlet;
  let broadcast_outlet = broadcast.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(broadcast), StageDefinition::Sink(sink)],
    vec![
      (source_outlet.id(), broadcast_inlet, MatCombine::KeepLeft),
      (broadcast_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let result = catch_unwind(AssertUnwindSafe(|| {
    let _ = GraphInterpreter::new(plan, StreamBufferConfig::default());
  }));
  assert!(result.is_err());
}

#[test]
fn balance_flow_distributes_elements_round_robin() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let right_inlet: Inlet<u32> = Inlet::new();
  let right_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 4 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let balance = balance_definition::<u32>(2);
  let balance_inlet = balance.inlet;
  let balance_outlet = balance.outlet;
  let right_flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       right_inlet.id(),
    outlet:      right_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(AddFlowLogic { add: 100 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SumSinkLogic { completion: completion.clone(), sum: 0 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![
      StageDefinition::Source(source),
      StageDefinition::Flow(balance),
      StageDefinition::Flow(right_flow),
      StageDefinition::Sink(sink),
    ],
    vec![
      (source_outlet.id(), balance_inlet, MatCombine::KeepLeft),
      (balance_outlet, sink_inlet.id(), MatCombine::KeepRight),
      (balance_outlet, right_inlet.id(), MatCombine::KeepLeft),
      (right_outlet.id(), sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(210)));
}

#[test]
fn rejects_balance_flow_when_fan_out_does_not_match_wiring() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SingleValueSourceLogic { value: Some(1_u32) }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let balance = balance_definition::<u32>(2);
  let balance_inlet = balance.inlet;
  let balance_outlet = balance.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(balance), StageDefinition::Sink(sink)],
    vec![
      (source_outlet.id(), balance_inlet, MatCombine::KeepLeft),
      (balance_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let result = catch_unwind(AssertUnwindSafe(|| {
    let _ = GraphInterpreter::new(plan, StreamBufferConfig::default());
  }));
  assert!(result.is_err());
}

#[test]
fn merge_flow_combines_multiple_incoming_edges() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let left_inlet: Inlet<u32> = Inlet::new();
  let left_outlet: Outlet<u32> = Outlet::new();
  let right_inlet: Inlet<u32> = Inlet::new();
  let right_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 4 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let left_flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       left_inlet.id(),
    outlet:      left_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(AddFlowLogic { add: 10 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let right_flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       right_inlet.id(),
    outlet:      right_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(AddFlowLogic { add: 100 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let merge = merge_definition::<u32>(2);
  let merge_inlet = merge.inlet;
  let merge_outlet = merge.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SumSinkLogic { completion: completion.clone(), sum: 0 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![
      StageDefinition::Source(source),
      StageDefinition::Flow(left_flow),
      StageDefinition::Flow(right_flow),
      StageDefinition::Flow(merge),
      StageDefinition::Sink(sink),
    ],
    vec![
      (source_outlet.id(), left_inlet.id(), MatCombine::KeepLeft),
      (source_outlet.id(), right_inlet.id(), MatCombine::KeepLeft),
      (left_outlet.id(), merge_inlet, MatCombine::KeepLeft),
      (right_outlet.id(), merge_inlet, MatCombine::KeepLeft),
      (merge_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(230)));
}

#[test]
fn rejects_merge_flow_when_fan_in_does_not_match_wiring() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SingleValueSourceLogic { value: Some(1_u32) }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let merge = merge_definition::<u32>(2);
  let merge_inlet = merge.inlet;
  let merge_outlet = merge.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(merge), StageDefinition::Sink(sink)],
    vec![
      (source_outlet.id(), merge_inlet, MatCombine::KeepLeft),
      (merge_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let result = catch_unwind(AssertUnwindSafe(|| {
    let _ = GraphInterpreter::new(plan, StreamBufferConfig::default());
  }));
  assert!(result.is_err());
}

#[test]
fn zip_flow_combines_elements_when_all_inputs_have_values() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let left_inlet: Inlet<u32> = Inlet::new();
  let left_outlet: Outlet<u32> = Outlet::new();
  let right_inlet: Inlet<u32> = Inlet::new();
  let right_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<Vec<u32>> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 4 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let left_flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       left_inlet.id(),
    outlet:      left_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(AddFlowLogic { add: 10 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let right_flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       right_inlet.id(),
    outlet:      right_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(AddFlowLogic { add: 100 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let zip = zip_definition::<u32>(2);
  let zip_inlet = zip.inlet;
  let zip_outlet = zip.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<Vec<u32>>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(ZipSumSinkLogic { completion: completion.clone(), sum: 0 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![
      StageDefinition::Source(source),
      StageDefinition::Flow(left_flow),
      StageDefinition::Flow(right_flow),
      StageDefinition::Flow(zip),
      StageDefinition::Sink(sink),
    ],
    vec![
      (source_outlet.id(), left_inlet.id(), MatCombine::KeepLeft),
      (source_outlet.id(), right_inlet.id(), MatCombine::KeepLeft),
      (left_outlet.id(), zip_inlet, MatCombine::KeepLeft),
      (right_outlet.id(), zip_inlet, MatCombine::KeepLeft),
      (zip_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(230)));
}

#[test]
fn rejects_zip_flow_when_fan_in_does_not_match_wiring() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<Vec<u32>> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SingleValueSourceLogic { value: Some(1_u32) }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let zip = zip_definition::<u32>(2);
  let zip_inlet = zip.inlet;
  let zip_outlet = zip.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<Vec<u32>>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(zip), StageDefinition::Sink(sink)],
    vec![(source_outlet.id(), zip_inlet, MatCombine::KeepLeft), (zip_outlet, sink_inlet.id(), MatCombine::KeepRight)],
  );

  let result = catch_unwind(AssertUnwindSafe(|| {
    let _ = GraphInterpreter::new(plan, StreamBufferConfig::default());
  }));
  assert!(result.is_err());
}

#[test]
fn concat_flow_emits_elements_in_input_order() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let left_inlet: Inlet<u32> = Inlet::new();
  let left_outlet: Outlet<u32> = Outlet::new();
  let right_inlet: Inlet<u32> = Inlet::new();
  let right_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SequenceSourceLogic { next: 1, end: 4 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let left_flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       left_inlet.id(),
    outlet:      left_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(AddFlowLogic { add: 10 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let right_flow = FlowDefinition {
    kind:        StageKind::FlowMap,
    inlet:       right_inlet.id(),
    outlet:      right_outlet.id(),
    input_type:  TypeId::of::<u32>(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepLeft,
    logic:       Box::new(AddFlowLogic { add: 100 }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let concat = concat_definition::<u32>(2);
  let concat_inlet = concat.inlet;
  let concat_outlet = concat.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkFold,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(CollectSequenceSinkLogic { completion: completion.clone(), values: Vec::new() }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![
      StageDefinition::Source(source),
      StageDefinition::Flow(left_flow),
      StageDefinition::Flow(right_flow),
      StageDefinition::Flow(concat),
      StageDefinition::Sink(sink),
    ],
    vec![
      (source_outlet.id(), left_inlet.id(), MatCombine::KeepLeft),
      (source_outlet.id(), right_inlet.id(), MatCombine::KeepLeft),
      (left_outlet.id(), concat_inlet, MatCombine::KeepLeft),
      (right_outlet.id(), concat_inlet, MatCombine::KeepLeft),
      (concat_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(interpreter.state(), StreamState::Completed);
  assert_eq!(completion.poll(), Completion::Ready(Ok(vec![11, 13, 102, 104])));
}

#[test]
fn rejects_concat_flow_when_fan_in_does_not_match_wiring() {
  let source_outlet: Outlet<u32> = Outlet::new();
  let sink_inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();

  let source = SourceDefinition {
    kind:        StageKind::SourceSingle,
    outlet:      source_outlet.id(),
    output_type: TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(SingleValueSourceLogic { value: Some(1_u32) }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };
  let concat = concat_definition::<u32>(2);
  let concat_inlet = concat.inlet;
  let concat_outlet = concat.outlet;
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion }),
    supervision: SupervisionStrategy::Stop,
    restart:     None,
  };

  let plan = StreamPlan::from_parts(
    vec![StageDefinition::Source(source), StageDefinition::Flow(concat), StageDefinition::Sink(sink)],
    vec![
      (source_outlet.id(), concat_inlet, MatCombine::KeepLeft),
      (concat_outlet, sink_inlet.id(), MatCombine::KeepRight),
    ],
  );

  let result = catch_unwind(AssertUnwindSafe(|| {
    let _ = GraphInterpreter::new(plan, StreamBufferConfig::default());
  }));
  assert!(result.is_err());
}

struct CountingSourceLogic {
  remaining: u32,
  pulls:     ArcShared<SpinSyncMutex<u32>>,
}

struct CancelAwareSequenceSourceLogic {
  next:    u32,
  end:     u32,
  pulls:   ArcShared<SpinSyncMutex<u32>>,
  cancels: ArcShared<SpinSyncMutex<u32>>,
}

impl SourceLogic for CancelAwareSequenceSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    *self.pulls.lock() += 1;
    if self.next > self.end {
      return Ok(None);
    }
    let value = self.next;
    self.next = self.next.saturating_add(1);
    Ok(Some(Box::new(value)))
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    *self.cancels.lock() += 1;
    Ok(())
  }
}

impl SourceLogic for CountingSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    *self.pulls.lock() += 1;
    if self.remaining == 0 {
      Ok(None)
    } else {
      self.remaining -= 1;
      Ok(Some(Box::new(1_u32)))
    }
  }
}

struct SingleValueSourceLogic {
  value: Option<u32>,
}

impl SourceLogic for SingleValueSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Ok(self.value.take().map(|value| Box::new(value) as DynValue))
  }
}

struct SequenceSourceLogic {
  next: u32,
  end:  u32,
}

impl SourceLogic for SequenceSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    if self.next > self.end {
      return Ok(None);
    }
    let value = self.next;
    self.next = self.next.saturating_add(1);
    Ok(Some(Box::new(value)))
  }
}

struct AlwaysFailSourceLogic;

impl SourceLogic for AlwaysFailSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    Err(StreamError::Failed)
  }
}

struct RestartableSingleSourceLogic {
  value:   u32,
  emitted: bool,
}

impl SourceLogic for RestartableSingleSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    if self.emitted {
      return Ok(None);
    }
    self.emitted = true;
    Ok(Some(Box::new(self.value)))
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.emitted = false;
    Ok(())
  }
}

struct NoDemandSinkLogic {
  completion: StreamCompletion<StreamDone>,
}

impl SinkLogic for NoDemandSinkLogic {
  fn on_start(&mut self, _demand: &mut DemandTracker) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_push(&mut self, _input: DynValue, _demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.completion.complete(Ok(StreamDone::new()));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

struct BlockedSinkLogic {
  completion: StreamCompletion<StreamDone>,
}

impl SinkLogic for BlockedSinkLogic {
  fn can_accept_input(&self) -> bool {
    false
  }

  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(8)
  }

  fn on_push(&mut self, _input: DynValue, _demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.completion.complete(Ok(StreamDone::new()));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

struct RecordingSinkLogic {
  completion: StreamCompletion<StreamDone>,
}

impl SinkLogic for RecordingSinkLogic {
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, _input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.completion.complete(Ok(StreamDone::new()));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

struct MismatchFlowLogic;

impl FlowLogic for MismatchFlowLogic {
  fn apply(&mut self, _input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    Ok(vec![Box::new("mismatch".to_string())])
  }
}

struct IncrementFlowLogic;

impl FlowLogic for IncrementFlowLogic {
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = *input.downcast::<u32>().map_err(|_| StreamError::TypeMismatch)?;
    Ok(vec![Box::new(value + 1)])
  }
}

struct AddFlowLogic {
  add: u32,
}

impl FlowLogic for AddFlowLogic {
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = *input.downcast::<u32>().map_err(|_| StreamError::TypeMismatch)?;
    Ok(vec![Box::new(value + self.add)])
  }
}

struct RestartCounterFlowLogic {
  restart_calls: ArcShared<SpinSyncMutex<u32>>,
}

impl FlowLogic for RestartCounterFlowLogic {
  fn apply(&mut self, _input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    Err(StreamError::Failed)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    let mut restart_calls = self.restart_calls.lock();
    *restart_calls = restart_calls.saturating_add(1);
    Ok(())
  }
}

struct SumSinkLogic {
  completion: StreamCompletion<u32>,
  sum:        u32,
}

impl SinkLogic for SumSinkLogic {
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = *input.downcast::<u32>().map_err(|_| StreamError::TypeMismatch)?;
    self.sum = self.sum.saturating_add(value);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.completion.complete(Ok(self.sum));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

struct ZipSumSinkLogic {
  completion: StreamCompletion<u32>,
  sum:        u32,
}

impl SinkLogic for ZipSumSinkLogic {
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    let values = *input.downcast::<Vec<u32>>().map_err(|_| StreamError::TypeMismatch)?;
    let pair_sum = values.into_iter().fold(0_u32, |acc, value| acc.saturating_add(value));
    self.sum = self.sum.saturating_add(pair_sum);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.completion.complete(Ok(self.sum));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

struct CollectSequenceSinkLogic {
  completion: StreamCompletion<Vec<u32>>,
  values:     Vec<u32>,
}

impl SinkLogic for CollectSequenceSinkLogic {
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = *input.downcast::<u32>().map_err(|_| StreamError::TypeMismatch)?;
    self.values.push(value);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    let values = core::mem::take(&mut self.values);
    self.completion.complete(Ok(values));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}

struct CollectNestedSequenceSinkLogic {
  completion: StreamCompletion<Vec<Vec<u32>>>,
  values:     Vec<Vec<u32>>,
}

impl SinkLogic for CollectNestedSequenceSinkLogic {
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = *input.downcast::<Vec<u32>>().map_err(|_| StreamError::TypeMismatch)?;
    self.values.push(value);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    let values = core::mem::take(&mut self.values);
    self.completion.complete(Ok(values));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Err(error));
  }
}
