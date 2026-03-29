use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::map_definition;
use crate::core::{
  DownstreamCancelAction, DynValue, FlowLogic, StageDefinition, StreamDone, StreamError,
  buffer::StreamBufferConfig,
  downcast_value,
  lifecycle::{DriveOutcome, Stream},
  mat::MatCombine,
  materialization::{Completion, StreamCompletion},
  queue::{QueueOfferResult, SourceQueue},
  shape::{Inlet, Outlet},
  stage::{Sink, Source},
};

const IDLE_BUDGET: usize = 32;
const DRIVE_BUDGET: usize = 256;

pub(in crate::core) struct SecondarySourceBridge<Out> {
  stream:     Stream,
  completion: StreamCompletion<StreamDone>,
  queue:      SourceQueue<Out>,
  finished:   bool,
}

impl<Out> SecondarySourceBridge<Out>
where
  Out: Send + Sync + 'static,
{
  fn sync_terminal_state(&mut self) -> Result<(), StreamError> {
    match self.completion.try_take() {
      | Some(Ok(_)) => {
        // completion が正常に取得できた → 終了
        self.finished = true;
        let _ = self.queue.complete_if_open();
        Ok(())
      },
      | None => {
        // まだ completion が来ていない → stream の状態を確認
        if self.stream.state().is_terminal() || matches!(self.completion.poll(), Completion::Ready(Ok(_))) {
          self.finished = true;
          let _ = self.queue.complete_if_open();
        }
        Ok(())
      },
      | Some(Err(error)) => {
        self.finished = true;
        self.queue.fail(error.clone());
        Err(error)
      },
    }
  }

  fn finalize_if_terminal(&mut self) -> Result<Option<Out>, StreamError> {
    if !self.stream.state().is_terminal() {
      return Ok(None);
    }
    self.sync_terminal_state()?;
    self.queue.poll()
  }

  fn refresh_after_emit(&mut self) -> Result<(), StreamError> {
    if self.finished || !self.queue.is_empty() {
      return Ok(());
    }

    self.sync_terminal_state()?;
    if self.finished || !self.queue.is_empty() {
      return Ok(());
    }

    let mut idle_budget = IDLE_BUDGET;
    let mut drive_budget = DRIVE_BUDGET;
    loop {
      if drive_budget == 0 {
        return Ok(());
      }
      drive_budget = drive_budget.saturating_sub(1);

      match self.stream.drive() {
        | DriveOutcome::Progressed => idle_budget = IDLE_BUDGET,
        | DriveOutcome::Idle => {
          if idle_budget == 0 {
            return Ok(());
          }
          idle_budget = idle_budget.saturating_sub(1);
        },
      }

      if !self.queue.is_empty() {
        return Ok(());
      }

      self.sync_terminal_state()?;
      if self.finished {
        return Ok(());
      }
    }
  }

  pub(in crate::core) fn new<Mat>(source: Source<Out, Mat>) -> Result<Self, StreamError>
  where
    Mat: Send + 'static, {
    let (mut graph, _mat) = source.into_parts();
    let Some(tail_outlet_id) = graph.tail_outlet() else {
      return Err(StreamError::InvalidConnection);
    };
    let tail_outlet = Outlet::<Out>::from_id(tail_outlet_id);
    let queue = SourceQueue::new();
    let mut sink_queue = queue.clone();
    let sink = Sink::foreach(move |value: Out| match sink_queue.offer(value) {
      | QueueOfferResult::Enqueued => {},
      | QueueOfferResult::Failure(error) => sink_queue.fail(error),
      | QueueOfferResult::Dropped => sink_queue.fail(StreamError::BufferOverflow),
      | QueueOfferResult::QueueClosed => sink_queue.fail(StreamError::Failed),
    });
    let (sink_graph, completion) = sink.into_parts();
    let Some(sink_inlet_id) = sink_graph.head_inlet() else {
      return Err(StreamError::InvalidConnection);
    };
    graph.append(sink_graph);
    let sink_inlet = Inlet::<Out>::from_id(sink_inlet_id);

    if let Some(expected_fan_out) = graph.expected_fan_out_for_outlet(tail_outlet_id) {
      for _ in 1..expected_fan_out {
        // ブロードキャストのファンアウト配線に必要なダミーパススルーステージ
        let branch = map_definition::<Out, Out, _>(|value| value);
        let branch_inlet = Inlet::<Out>::from_id(branch.inlet);
        let branch_outlet = Outlet::<Out>::from_id(branch.outlet);
        graph.push_stage(StageDefinition::Flow(branch));
        graph.connect(&tail_outlet, &branch_inlet, MatCombine::Left)?;
        graph.connect(&branch_outlet, &sink_inlet, MatCombine::Right)?;
      }
    }

    let plan = graph.into_plan()?;
    let mut stream = Stream::new(plan, StreamBufferConfig::default());
    stream.start()?;
    Ok(Self { stream, completion, queue, finished: false })
  }

  pub(in crate::core) fn poll_next(&mut self) -> Result<Option<Out>, StreamError> {
    if let Some(value) = self.queue.poll()? {
      if self.queue.is_empty() {
        self.refresh_after_emit()?;
      }
      return Ok(Some(value));
    }
    if self.finished {
      return Ok(None);
    }

    let mut idle_budget = IDLE_BUDGET;
    let mut drive_budget = DRIVE_BUDGET;
    loop {
      match self.finalize_if_terminal()? {
        | Some(value) => return Ok(Some(value)),
        | None if self.finished => return Ok(None),
        | None => {},
      }
      if drive_budget == 0 {
        return Ok(None);
      }
      drive_budget = drive_budget.saturating_sub(1);

      match self.stream.drive() {
        | DriveOutcome::Progressed => idle_budget = IDLE_BUDGET,
        | DriveOutcome::Idle => {
          if idle_budget == 0 {
            return Ok(None);
          }
          idle_budget = idle_budget.saturating_sub(1);
        },
      }

      if let Some(value) = self.queue.poll()? {
        return Ok(Some(value));
      }
    }
  }

  pub(in crate::core) fn has_pending_output(&self) -> bool {
    !self.queue.is_empty() || !self.finished
  }
}

pub(in crate::core) struct ConcatSourceLogic<Out, Mat> {
  pub(in crate::core) secondary:         Option<Source<Out, Mat>>,
  pub(in crate::core) secondary_runtime: Option<SecondarySourceBridge<Out>>,
  pub(in crate::core) pending:           VecDeque<Out>,
  pub(in crate::core) source_done:       bool,
}

impl<Out, Mat> FlowLogic for ConcatSourceLogic<Out, Mat>
where
  Out: Send + Sync + 'static,
  Mat: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<Out>(input)?;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    Ok(())
  }

  fn on_downstream_cancel(&mut self) -> Result<DownstreamCancelAction, StreamError> {
    self.on_source_done()?;
    self.pending.clear();
    self.secondary_runtime = None;
    Ok(DownstreamCancelAction::Propagate)
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done {
      return Ok(Vec::new());
    }

    if self.secondary_runtime.is_none()
      && let Some(source) = self.secondary.take()
    {
      self.secondary_runtime = Some(SecondarySourceBridge::new(source)?);
    }
    if let Some(runtime) = self.secondary_runtime.as_mut()
      && let Some(value) = runtime.poll_next()?
    {
      self.pending.push_back(value);
    }
    let Some(value) = self.pending.pop_front() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(value) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    self.source_done
      && (!self.pending.is_empty()
        || self.secondary.is_some()
        || self.secondary_runtime.as_ref().is_some_and(SecondarySourceBridge::has_pending_output))
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending.clear();
    self.source_done = false;
    if self.secondary.is_some() {
      self.secondary_runtime = None;
    }
    Ok(())
  }
}

pub(in crate::core) struct PrependSourceLogic<Out, Mat> {
  pub(in crate::core) secondary:         Option<Source<Out, Mat>>,
  pub(in crate::core) secondary_runtime: Option<SecondarySourceBridge<Out>>,
  pub(in crate::core) pending_secondary: VecDeque<Out>,
  pub(in crate::core) pending_primary:   VecDeque<Out>,
}

impl<Out, Mat> FlowLogic for PrependSourceLogic<Out, Mat>
where
  Out: Send + Sync + 'static,
  Mat: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<Out>(input)?;
    if self.secondary_runtime.is_none()
      && let Some(source) = self.secondary.take()
    {
      self.secondary_runtime = Some(SecondarySourceBridge::new(source)?);
    }
    self.pending_primary.push_back(value);
    self.drain_pending()
  }

  fn can_accept_input(&self) -> bool {
    self.pending_primary.is_empty()
  }

  fn on_downstream_cancel(&mut self) -> Result<DownstreamCancelAction, StreamError> {
    self.on_source_done()?;
    self.pending_primary.clear();
    self.pending_secondary.clear();
    self.secondary_runtime = None;
    Ok(DownstreamCancelAction::Propagate)
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if self.secondary_runtime.is_none()
      && let Some(source) = self.secondary.take()
    {
      self.secondary_runtime = Some(SecondarySourceBridge::new(source)?);
    }
    if let Some(runtime) = self.secondary_runtime.as_mut()
      && let Some(value) = runtime.poll_next()?
    {
      self.pending_secondary.push_back(value);
    }

    if let Some(value) = self.pending_secondary.pop_front() {
      return Ok(vec![Box::new(value) as DynValue]);
    }

    let Some(value) = self.pending_primary.pop_front() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(value) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    !self.pending_secondary.is_empty()
      || !self.pending_primary.is_empty()
      || self.secondary.is_some()
      || self.secondary_runtime.as_ref().is_some_and(SecondarySourceBridge::has_pending_output)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending_primary.clear();
    self.pending_secondary.clear();
    if self.secondary.is_some() {
      self.secondary_runtime = None;
    }
    Ok(())
  }
}

pub(in crate::core) struct OrElseSourceLogic<Out, Mat> {
  pub(in crate::core) secondary:         Option<Source<Out, Mat>>,
  pub(in crate::core) secondary_runtime: Option<SecondarySourceBridge<Out>>,
  pub(in crate::core) pending_secondary: VecDeque<Out>,
  pub(in crate::core) emitted_primary:   bool,
  pub(in crate::core) source_done:       bool,
}

impl<Out, Mat> FlowLogic for OrElseSourceLogic<Out, Mat>
where
  Out: Send + Sync + 'static,
  Mat: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<Out>(input)?;
    self.emitted_primary = true;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    if self.emitted_primary {
      self.pending_secondary.clear();
      return Ok(());
    }
    Ok(())
  }

  fn on_downstream_cancel(&mut self) -> Result<DownstreamCancelAction, StreamError> {
    self.on_source_done()?;
    self.pending_secondary.clear();
    self.secondary_runtime = None;
    Ok(DownstreamCancelAction::Propagate)
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if !self.source_done || self.emitted_primary {
      return Ok(Vec::new());
    }
    if self.secondary_runtime.is_none()
      && let Some(source) = self.secondary.take()
    {
      self.secondary_runtime = Some(SecondarySourceBridge::new(source)?);
    }
    if let Some(runtime) = self.secondary_runtime.as_mut()
      && let Some(value) = runtime.poll_next()?
    {
      self.pending_secondary.push_back(value);
    }
    let Some(value) = self.pending_secondary.pop_front() else {
      return Ok(Vec::new());
    };
    Ok(vec![Box::new(value) as DynValue])
  }

  fn has_pending_output(&self) -> bool {
    self.source_done
      && !self.emitted_primary
      && (!self.pending_secondary.is_empty()
        || self.secondary.is_some()
        || self.secondary_runtime.as_ref().is_some_and(SecondarySourceBridge::has_pending_output))
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending_secondary.clear();
    self.emitted_primary = false;
    self.source_done = false;
    if self.secondary.is_some() {
      self.secondary_runtime = None;
    }
    Ok(())
  }
}
