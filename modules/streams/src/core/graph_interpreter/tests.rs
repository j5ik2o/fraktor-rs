use core::any::TypeId;

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use crate::core::{
  Completion, DemandTracker, DriveOutcome, DynValue, FlowDefinition, FlowLogic, GraphInterpreter, Inlet, KeepRight,
  MatCombine, Outlet, Sink, SinkDecision, SinkDefinition, SinkLogic, Source, SourceDefinition, SourceLogic, StageKind,
  StreamBufferConfig, StreamCompletion, StreamDone, StreamError, StreamPlan, StreamState,
};

fn drive_to_completion(interpreter: &mut GraphInterpreter) {
  interpreter.start().expect("start");
  while interpreter.state() == StreamState::Running {
    let _ = interpreter.drive();
  }
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
fn flat_map_concat_uses_inner_source() {
  let graph = Source::single(1_u32).flat_map_concat(|value| Source::single(value + 1)).to_mat(Sink::head(), KeepRight);
  let (plan, completion) = graph.into_parts();
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  drive_to_completion(&mut interpreter);
  assert_eq!(completion.poll(), Completion::Ready(Ok(2)));
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
  };
  let inlet: Inlet<u32> = Inlet::new();
  let completion = StreamCompletion::new();
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(NoDemandSinkLogic { completion }),
  };
  let plan = StreamPlan { source, flows: Vec::new(), sink };
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
  };
  let sink_inlet: Inlet<u32> = Inlet::new();
  let sink = SinkDefinition {
    kind:        StageKind::SinkIgnore,
    inlet:       sink_inlet.id(),
    input_type:  TypeId::of::<u32>(),
    mat_combine: MatCombine::KeepRight,
    logic:       Box::new(RecordingSinkLogic { completion: completion.clone() }),
  };
  let plan = StreamPlan { source, flows: vec![flow], sink };
  let mut interpreter = GraphInterpreter::new(plan, StreamBufferConfig::default());
  interpreter.start().expect("start");
  while interpreter.state() == StreamState::Running {
    let _ = interpreter.drive();
  }
  assert_eq!(interpreter.state(), StreamState::Failed);
  assert_eq!(completion.poll(), Completion::Ready(Err(StreamError::TypeMismatch)));
}

struct CountingSourceLogic {
  remaining: u32,
  pulls:     ArcShared<SpinSyncMutex<u32>>,
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
