#[cfg(test)]
mod tests;

extern crate std;

use std::sync::{
  atomic::{AtomicUsize, Ordering},
  mpsc,
};

use crate::core::{
  collections::queue::{OfferOutcome, OverflowPolicy, QueueError, SyncQueueBackend, backend::SyncQueueBackendInternal},
  sync::ArcShared,
};

/// Queue backend using [`std::sync::mpsc`] channel.
///
/// This backend provides unbounded queue semantics backed by the standard library's
/// multi-producer, single-consumer channel. It supports concurrent producers and
/// maintains approximate length tracking via atomic counters.
pub struct MpscBackend<T> {
  sender:   std::sync::mpsc::Sender<T>,
  receiver: std::sync::mpsc::Receiver<T>,
  len:      ArcShared<AtomicUsize>,
}

impl<T> MpscBackend<T> {
  /// Creates a new unbounded MPSC backend.
  #[must_use]
  pub fn new() -> Self {
    let (sender, receiver) = mpsc::channel();
    Self { sender, receiver, len: ArcShared::new(AtomicUsize::new(0)) }
  }

  /// Returns a reference to the sender half of the channel.
  ///
  /// This can be cloned to create multiple producers.
  #[must_use]
  pub const fn sender(&self) -> &std::sync::mpsc::Sender<T> {
    &self.sender
  }

  /// Returns a reference to the receiver half of the channel.
  #[must_use]
  pub const fn receiver(&self) -> &std::sync::mpsc::Receiver<T> {
    &self.receiver
  }
}

impl<T> Default for MpscBackend<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> SyncQueueBackendInternal<T> for MpscBackend<T> {
  fn offer(&mut self, item: T) -> Result<OfferOutcome, QueueError<T>> {
    match self.sender.send(item) {
      | Ok(()) => {
        self.len.fetch_add(1, Ordering::Relaxed);
        Ok(OfferOutcome::Enqueued)
      },
      | Err(mpsc::SendError(_item)) => Err(QueueError::Disconnected),
    }
  }

  fn poll(&mut self) -> Result<T, QueueError<T>> {
    match self.receiver.try_recv() {
      | Ok(item) => {
        self.len.fetch_sub(1, Ordering::Relaxed);
        Ok(item)
      },
      | Err(mpsc::TryRecvError::Empty) => Err(QueueError::Empty),
      | Err(mpsc::TryRecvError::Disconnected) => Err(QueueError::Disconnected),
    }
  }

  fn len(&self) -> usize {
    self.len.load(Ordering::Relaxed)
  }

  fn capacity(&self) -> usize {
    // MPSC channels are unbounded
    usize::MAX
  }

  fn overflow_policy(&self) -> OverflowPolicy {
    // MPSC channels grow unbounded
    OverflowPolicy::Grow
  }
}

impl<T> SyncQueueBackend<T> for MpscBackend<T> {}
