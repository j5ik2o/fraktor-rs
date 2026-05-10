use core::marker::PhantomData;

use crate::{
  DemandTracker, DynValue, QueueOfferResult, SinkDecision, SinkLogic, StreamError, downcast_value,
  r#impl::queue::SourceQueue,
};

/// Sink logic that forwards each input element into a materialized source.
pub(crate) struct SinkSourceLogic<In> {
  queue: SourceQueue<In>,
  _pd:   PhantomData<fn(In)>,
}

impl<In> SinkSourceLogic<In> {
  /// Creates sink-source bridge logic.
  pub(crate) const fn new(queue: SourceQueue<In>) -> Self {
    Self { queue, _pd: PhantomData }
  }
}

impl<In> SinkLogic for SinkSourceLogic<In>
where
  In: Send + Sync + 'static,
{
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = downcast_value::<In>(input)?;
    match self.queue.offer(value) {
      | QueueOfferResult::Enqueued | QueueOfferResult::Dropped => {
        demand.request(1)?;
        Ok(SinkDecision::Continue)
      },
      | QueueOfferResult::QueueClosed => Ok(SinkDecision::Complete),
      | QueueOfferResult::Failure(error) => Err(error),
    }
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    self.queue.complete();
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.queue.fail(error);
  }

  fn on_upstream_finish(&mut self) -> Result<bool, StreamError> {
    self.queue.complete();
    Ok(false)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.queue.close_for_cancel();
    Ok(())
  }
}
