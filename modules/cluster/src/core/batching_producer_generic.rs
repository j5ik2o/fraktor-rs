//! Batching producer for pub/sub publishes.

#[cfg(test)]
mod tests;

use alloc::{format, vec::Vec};

use fraktor_actor_rs::core::{
  messaging::AnyMessageGeneric,
  scheduler::{ExecutionBatch, SchedulerCommand, SchedulerRunnable, SchedulerSharedGeneric},
};
use fraktor_utils_rs::core::{
  collections::queue::{OverflowPolicy, QueueError, SyncFifoQueue, backend::VecDequeBackend},
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, SharedAccess, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  BatchingProducerConfig, PubSubError, PubSubPublisherGeneric, PubSubTopic, PublishAck, PublishOptions,
  PublishRejectReason,
};

/// Batching producer handle.
pub struct BatchingProducerGeneric<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<BatchingProducerInner<TB>>,
}

impl<TB: RuntimeToolbox + 'static> Clone for BatchingProducerGeneric<TB> {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone() }
  }
}

impl<TB: RuntimeToolbox + 'static> BatchingProducerGeneric<TB> {
  /// Creates a new batching producer.
  #[must_use]
  pub fn new(
    topic: PubSubTopic,
    publisher: PubSubPublisherGeneric<TB>,
    scheduler: SchedulerSharedGeneric<TB>,
    config: BatchingProducerConfig,
  ) -> Self {
    let state = BatchingProducerState::new(config.max_queue_size);
    let inner = BatchingProducerInner {
      state: <TB::MutexFamily as SyncMutexFamily>::create(state),
      topic,
      publisher,
      scheduler,
      config,
    };
    Self { inner: ArcShared::new(inner) }
  }

  /// Enqueues a message and triggers flush when batch conditions are met.
  ///
  /// # Errors
  ///
  /// Returns `PubSubError` for system-level failures.
  pub fn produce(&self, message: AnyMessageGeneric<TB>) -> Result<PublishAck, PubSubError> {
    let mut schedule = false;
    let mut flush_messages: Vec<AnyMessageGeneric<TB>> = Vec::new();

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

  fn flush_messages(&self, messages: Vec<AnyMessageGeneric<TB>>) -> Result<PublishAck, PubSubError> {
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

struct BatchingProducerInner<TB: RuntimeToolbox + 'static> {
  state:     ToolboxMutex<BatchingProducerState<TB>, TB>,
  topic:     PubSubTopic,
  publisher: PubSubPublisherGeneric<TB>,
  scheduler: SchedulerSharedGeneric<TB>,
  config:    BatchingProducerConfig,
}

struct BatchingProducerState<TB: RuntimeToolbox> {
  queue:        SyncFifoQueue<AnyMessageGeneric<TB>, VecDequeBackend<AnyMessageGeneric<TB>>>,
  timer_active: bool,
}

impl<TB: RuntimeToolbox> BatchingProducerState<TB> {
  fn new(capacity: usize) -> Self {
    let backend = VecDequeBackend::with_capacity(capacity, OverflowPolicy::Block);
    Self { queue: SyncFifoQueue::new(backend), timer_active: false }
  }

  fn drain_batch(&mut self, max: usize) -> Vec<AnyMessageGeneric<TB>> {
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

struct BatchFlushRunnable<TB: RuntimeToolbox + 'static> {
  producer: BatchingProducerGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> SchedulerRunnable for BatchFlushRunnable<TB> {
  fn run(&self, _batch: &ExecutionBatch) {
    self.producer.on_timer();
  }
}
