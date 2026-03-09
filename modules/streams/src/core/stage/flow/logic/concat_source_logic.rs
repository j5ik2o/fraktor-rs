use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, Sink, Source, StreamError, downcast_value};
use crate::core::{
  KeepRight, QueueOfferResult, SourceQueue, StreamBufferConfig, StreamCompletion, StreamDone,
  lifecycle::{DriveOutcome, Stream},
};

pub(in crate::core::stage::flow) struct SecondarySourceBridge<Out> {
  stream:     Stream,
  completion: StreamCompletion<StreamDone>,
  queue:      SourceQueue<Out>,
  finished:   bool,
}

impl<Out> SecondarySourceBridge<Out>
where
  Out: Send + Sync + 'static,
{
  pub(in crate::core::stage::flow) fn new<Mat>(source: Source<Out, Mat>) -> Result<Self, StreamError>
  where
    Mat: Send + Sync + 'static, {
    let queue = SourceQueue::new();
    let sink_queue = queue.clone();
    let sink = Sink::foreach(move |value: Out| match sink_queue.offer(value) {
      | QueueOfferResult::Enqueued => {},
      | QueueOfferResult::Failure(error) => sink_queue.fail(error),
      | QueueOfferResult::Dropped | QueueOfferResult::QueueClosed => sink_queue.fail(StreamError::Failed),
    });
    let graph = source.to_mat(sink, KeepRight);
    let (plan, completion) = graph.into_parts();
    let mut stream = Stream::new(plan, StreamBufferConfig::default());
    stream.start()?;
    Ok(Self { stream, completion, queue, finished: false })
  }

  pub(in crate::core::stage::flow) fn poll_next(&mut self) -> Result<Option<Out>, StreamError> {
    if let Some(value) = self.queue.poll()? {
      return Ok(Some(value));
    }
    if self.finished {
      return Ok(None);
    }

    match self.stream.drive() {
      | DriveOutcome::Progressed | DriveOutcome::Idle => {},
    }

    if let Some(value) = self.queue.poll()? {
      return Ok(Some(value));
    }
    if self.stream.state().is_terminal() {
      self.finished = true;
      match self.completion.try_take() {
        | Some(Ok(_)) | None => {
          self.queue.complete();
          return self.queue.poll();
        },
        | Some(Err(error)) => {
          self.queue.fail(error.clone());
          return Err(error);
        },
      }
    }

    Ok(None)
  }

  pub(in crate::core::stage::flow) fn has_pending_output(&self) -> bool {
    !self.queue.is_empty() || !self.finished
  }
}

pub(in crate::core::stage::flow) struct ConcatSourceLogic<Out, Mat> {
  pub(in crate::core::stage::flow) secondary:         Option<Source<Out, Mat>>,
  pub(in crate::core::stage::flow) secondary_runtime: Option<SecondarySourceBridge<Out>>,
  pub(in crate::core::stage::flow) pending:           VecDeque<Out>,
  pub(in crate::core::stage::flow) source_done:       bool,
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

  fn on_downstream_cancel(&mut self) -> Result<(), StreamError> {
    self.pending.clear();
    self.secondary_runtime = None;
    Ok(())
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

pub(in crate::core::stage::flow) struct PrependSourceLogic<Out, Mat> {
  pub(in crate::core::stage::flow) secondary:         Option<Source<Out, Mat>>,
  pub(in crate::core::stage::flow) secondary_runtime: Option<SecondarySourceBridge<Out>>,
  pub(in crate::core::stage::flow) pending_secondary: VecDeque<Out>,
  pub(in crate::core::stage::flow) pending_primary:   VecDeque<Out>,
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

  fn on_downstream_cancel(&mut self) -> Result<(), StreamError> {
    self.pending_primary.clear();
    self.pending_secondary.clear();
    self.secondary_runtime = None;
    Ok(())
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

pub(in crate::core::stage::flow) struct OrElseSourceLogic<Out, Mat> {
  pub(in crate::core::stage::flow) secondary:         Option<Source<Out, Mat>>,
  pub(in crate::core::stage::flow) secondary_runtime: Option<SecondarySourceBridge<Out>>,
  pub(in crate::core::stage::flow) pending_secondary: VecDeque<Out>,
  pub(in crate::core::stage::flow) emitted_primary:   bool,
  pub(in crate::core::stage::flow) source_done:       bool,
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

  fn on_downstream_cancel(&mut self) -> Result<(), StreamError> {
    self.pending_secondary.clear();
    self.secondary_runtime = None;
    Ok(())
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
