//! Batching producer for pub/sub publishes.

#[cfg(test)]
mod tests;

use alloc::{format, vec::Vec};

use fraktor_actor_rs::core::kernel::{
  messaging::AnyMessage,
  scheduler::{ExecutionBatch, SchedulerCommand, SchedulerRunnable, SchedulerShared},
};
use fraktor_utils_rs::core::{
  collections::queue::{OverflowPolicy, QueueError, SyncFifoQueue, backend::VecDequeBackend},
  sync::{ArcShared, RuntimeMutex, SharedAccess},
};

use super::{
  BatchingProducerConfig, PubSubError, PubSubPublisher, PubSubTopic, PublishAck, PublishOptions, PublishRejectReason,
};

/// Batching producer handle.
pub struct BatchingProducer {
  inner: ArcShared<BatchingProducerInner>,
}

impl Clone for BatchingProducer {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl BatchingProducer {
  /// Creates a new batching producer.
  #[must_use]
  pub fn new(
    topic: PubSubTopic,
    publisher: PubSubPublisher,
    scheduler: SchedulerShared,
    config: BatchingProducerConfig,
  ) -> Self {
    let state = BatchingProducerState::new(config.max_queue_size);
    let inner = BatchingProducerInner { state: RuntimeMutex::new(state), topic, publisher, scheduler, config };
    Self { inner: ArcShared::new(inner) }
  }

  /// Enqueues a message and triggers flush when batch conditions are met.
  ///
  /// # Errors
  ///
  /// Returns `PubSubError` for system-level failures.
  pub fn produce(&self, message: AnyMessage) -> Result<PublishAck, PubSubError> {
    let mut schedule = false;
    let mut flush_messages: Vec<AnyMessage> = Vec::new();

    {
      let mut guard = self.inner.state.lock();
      match guard.queue.offer(message) {
        | Ok(_) => {
          if !guard.timer_active {
            guard.timer_active = true;
            schedule = true;
          }

          if guard.queue.len() >= self.inner.config.batch_size {
            flush_messages = guard.drain_batch(self.inner.config.batch_size);
            if guard.queue.is_empty() {
              guard.timer_active = false;
            }
          }
        },
        | Err(QueueError::Full(_)) => {
          return Ok(PublishAck::rejected(PublishRejectReason::QueueFull));
        },
        | Err(error) => {
          return Err(PubSubError::DeliveryFailed { reason: format!("{error:?}") });
        },
      }
    }

    if schedule {
      self.schedule_flush()?;
    }

    if !flush_messages.is_empty() {
      return self.flush_messages(flush_messages);
    }

    Ok(PublishAck::accepted())
  }

  /// Flushes the current queue contents immediately.
  ///
  /// # Errors
  ///
  /// Returns `PubSubError` for system-level failures.
  pub fn flush(&self) -> Result<PublishAck, PubSubError> {
    let mut batches = Vec::new();
    {
      let mut guard = self.inner.state.lock();
      guard.timer_active = false;
      while !guard.queue.is_empty() {
        batches.push(guard.drain_batch(self.inner.config.batch_size));
      }
    }

    let mut last_ack = PublishAck::accepted();
    for batch in batches {
      if batch.is_empty() {
        continue;
      }
      last_ack = self.flush_messages(batch)?;
    }
    Ok(last_ack)
  }

  fn schedule_flush(&self) -> Result<(), PubSubError> {
    let runnable = ArcShared::new(BatchFlushRunnable { producer: self.clone() });
    let command = SchedulerCommand::RunRunnable { runnable, dispatcher: None };
    let result =
      self.inner.scheduler.with_write(|scheduler| scheduler.schedule_once(self.inner.config.max_wait, command));
    if let Err(error) = result {
      let mut guard = self.inner.state.lock();
      guard.timer_active = false;
      return Err(PubSubError::DeliveryFailed { reason: format!("{error:?}") });
    }
    Ok(())
  }

  fn flush_messages(&self, messages: Vec<AnyMessage>) -> Result<PublishAck, PubSubError> {
    let batch = match self.inner.publisher.build_batch(messages) {
      | Ok(batch) => batch,
      | Err(reason) => return Ok(PublishAck::rejected(reason)),
    };
    self.inner.publisher.publish_batch(&self.inner.topic, batch, PublishOptions::default())
  }

  fn on_timer(&self) {
    let mut batches = Vec::new();
    {
      let mut guard = self.inner.state.lock();
      guard.timer_active = false;
      if guard.queue.is_empty() {
        return;
      }
      batches.push(guard.drain_batch(self.inner.config.batch_size));
      if !guard.queue.is_empty() {
        guard.timer_active = true;
      }
    }

    for batch in batches {
      if batch.is_empty() {
        continue;
      }
      let _ = self.flush_messages(batch);
    }

    if self.inner.state.lock().timer_active {
      let _ = self.schedule_flush();
    }
  }
}

struct BatchingProducerInner {
  state:     RuntimeMutex<BatchingProducerState>,
  topic:     PubSubTopic,
  publisher: PubSubPublisher,
  scheduler: SchedulerShared,
  config:    BatchingProducerConfig,
}

struct BatchingProducerState {
  queue:        SyncFifoQueue<AnyMessage, VecDequeBackend<AnyMessage>>,
  timer_active: bool,
}

impl BatchingProducerState {
  fn new(capacity: usize) -> Self {
    let backend = VecDequeBackend::with_capacity(capacity, OverflowPolicy::Block);
    Self { queue: SyncFifoQueue::new(backend), timer_active: false }
  }

  fn drain_batch(&mut self, max: usize) -> Vec<AnyMessage> {
    let mut items = Vec::new();
    for _ in 0..max {
      match self.queue.poll() {
        | Ok(item) => items.push(item),
        | Err(QueueError::Empty) => break,
        | Err(_) => break,
      }
    }
    items
  }
}

struct BatchFlushRunnable {
  producer: BatchingProducer,
}

impl SchedulerRunnable for BatchFlushRunnable {
  fn run(&self, _batch: &ExecutionBatch) {
    self.producer.on_timer();
  }
}
