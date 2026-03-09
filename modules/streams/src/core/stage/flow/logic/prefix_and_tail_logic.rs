use alloc::{boxed::Box, vec, vec::Vec};

use super::super::super::{
  DynValue, FlowLogic, Source, SourceQueue, StreamError, StreamNotUsed, TailSource, downcast_value,
};
use crate::core::QueueOfferResult;

pub(in crate::core::stage::flow) struct PrefixAndTailLogic<In> {
  pub(in crate::core::stage::flow) prefix_len:  usize,
  pub(in crate::core::stage::flow) prefix:      Vec<In>,
  pub(in crate::core::stage::flow) tail_source: Option<TailSource<In>>,
  pub(in crate::core::stage::flow) tail_queue:  SourceQueue<In>,
  pub(in crate::core::stage::flow) source_done: bool,
  pub(in crate::core::stage::flow) emitted:     bool,
}

impl<In> PrefixAndTailLogic<In>
where
  In: Send + Sync + 'static,
{
  pub(in crate::core::stage::flow) fn new(prefix_len: usize) -> Self {
    let (tail_source, tail_queue) = detached_tail_source::<In>();
    Self {
      prefix_len,
      prefix: Vec::new(),
      tail_source: Some(tail_source),
      tail_queue,
      source_done: false,
      emitted: false,
    }
  }

  fn emit_if_ready(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if self.emitted {
      return Ok(Vec::new());
    }
    if !self.source_done && self.prefix.len() < self.prefix_len {
      return Ok(Vec::new());
    }

    self.emitted = true;
    if self.source_done {
      self.tail_queue.complete();
    }
    let Some(tail_source) = self.tail_source.take() else {
      return Err(StreamError::Failed);
    };
    let output = (core::mem::take(&mut self.prefix), tail_source);
    Ok(vec![Box::new(output) as DynValue])
  }
}

impl<In> FlowLogic for PrefixAndTailLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    if !self.emitted && self.prefix.len() < self.prefix_len {
      self.prefix.push(value);
      return self.emit_if_ready();
    }

    match self.tail_queue.offer(value) {
      | QueueOfferResult::Enqueued => Ok(Vec::new()),
      | QueueOfferResult::Failure(error) => Err(error),
      | QueueOfferResult::Dropped | QueueOfferResult::QueueClosed => Err(StreamError::Failed),
    }
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    self.tail_queue.complete();
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    self.emit_if_ready()
  }

  fn has_pending_output(&self) -> bool {
    !self.emitted && (self.source_done || self.prefix.len() >= self.prefix_len)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.prefix.clear();
    self.source_done = false;
    self.emitted = false;
    let (tail_source, tail_queue) = detached_tail_source::<In>();
    self.tail_source = Some(tail_source);
    self.tail_queue = tail_queue;
    Ok(())
  }
}

fn detached_tail_source<In>() -> (TailSource<In>, SourceQueue<In>)
where
  In: Send + Sync + 'static, {
  let (graph, tail_queue) = Source::<In, _>::queue_unbounded().into_parts();
  (TailSource::new(Source::from_graph(graph, StreamNotUsed::new())), tail_queue)
}
