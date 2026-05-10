use alloc::{boxed::Box, vec, vec::Vec};

use crate::{
  DynValue, FlowLogic, QueueOfferResult, StreamError, downcast_value,
  dsl::{Source, TailSource},
  r#impl::queue::SourceQueue,
  materialization::StreamNotUsed,
};

pub(crate) struct PrefixAndTailLogic<In> {
  pub(crate) prefix_len:  usize,
  pub(crate) prefix:      Vec<In>,
  pub(crate) tail_source: Option<TailSource<In>>,
  pub(crate) tail_queue:  SourceQueue<In>,
  pub(crate) source_done: bool,
  pub(crate) emitted:     bool,
}

impl<In> PrefixAndTailLogic<In>
where
  In: Send + Sync + 'static,
{
  pub(crate) fn new(prefix_len: usize) -> Self {
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
      let _ = self.tail_queue.complete_if_open();
    }
    let Some(tail_source) = self.tail_source.take() else {
      return Err(StreamError::InvalidConnection);
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
    let _ = self.tail_queue.complete_if_open();
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
