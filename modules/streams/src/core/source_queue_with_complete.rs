use alloc::collections::VecDeque;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::{Completion, OverflowStrategy, QueueOfferResult, StreamCompletion, StreamDone, StreamError};

#[cfg(test)]
mod tests;

struct PendingOffer<T> {
  value:      T,
  completion: StreamCompletion<QueueOfferResult>,
}

struct QueueOfferFuture {
  completion: StreamCompletion<QueueOfferResult>,
}

impl Future for QueueOfferFuture {
  type Output = QueueOfferResult;

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
    match self.completion.poll_with_waker(context.waker()) {
      | Completion::Ready(Ok(result)) => Poll::Ready(result),
      | Completion::Ready(Err(error)) => Poll::Ready(QueueOfferResult::Failure(error)),
      | Completion::Pending => Poll::Pending,
    }
  }
}

struct SourceQueueWithCompleteState<T> {
  values:         VecDeque<T>,
  pending_offers: VecDeque<PendingOffer<T>>,
  closed:         bool,
  failure:        Option<StreamError>,
}

/// Queue materialized by `Source::queue_with_overflow`.
///
/// The handle provides asynchronous offer acknowledgements and completion watching.
pub struct SourceQueueWithComplete<T> {
  inner:                 ArcShared<SpinSyncMutex<SourceQueueWithCompleteState<T>>>,
  completion:            StreamCompletion<StreamDone>,
  capacity:              usize,
  overflow_strategy:     OverflowStrategy,
  max_concurrent_offers: usize,
}

impl<T> Clone for SourceQueueWithComplete<T> {
  fn clone(&self) -> Self {
    Self {
      inner:                 self.inner.clone(),
      completion:            self.completion.clone(),
      capacity:              self.capacity,
      overflow_strategy:     self.overflow_strategy,
      max_concurrent_offers: self.max_concurrent_offers,
    }
  }
}

impl<T> SourceQueueWithComplete<T> {
  /// Creates an empty queue with completion notifications.
  ///
  /// Use [`crate::core::Source::queue_with_overflow_and_max_concurrent_offers`] to validate
  /// `max_concurrent_offers` before constructing this queue.
  #[must_use]
  pub(crate) fn new(capacity: usize, overflow_strategy: OverflowStrategy, max_concurrent_offers: usize) -> Self {
    let state = SourceQueueWithCompleteState {
      values:         VecDeque::new(),
      pending_offers: VecDeque::new(),
      closed:         false,
      failure:        None,
    };
    Self {
      inner: ArcShared::new(SpinSyncMutex::new(state)),
      completion: StreamCompletion::new(),
      capacity,
      overflow_strategy,
      max_concurrent_offers,
    }
  }

  /// Offers a value into the queue and returns an asynchronous acknowledgement.
  pub fn offer(&self, value: T) -> impl Future<Output = QueueOfferResult> {
    QueueOfferFuture { completion: self.offer_now(value) }
  }

  fn offer_now(&self, value: T) -> StreamCompletion<QueueOfferResult> {
    let completion = StreamCompletion::new();
    let mut guard = self.inner.lock();
    if let Some(error) = &guard.failure {
      completion.complete(Ok(QueueOfferResult::Failure(error.clone())));
      return completion;
    }
    if guard.closed {
      completion.complete(Ok(QueueOfferResult::QueueClosed));
      return completion;
    }
    if self.capacity == 0 {
      return self.offer_without_buffer(&mut guard, value, completion);
    }
    if guard.values.len() < self.capacity {
      guard.values.push_back(value);
      completion.complete(Ok(QueueOfferResult::Enqueued));
      return completion;
    }

    match self.overflow_strategy {
      | OverflowStrategy::Backpressure => {
        if guard.pending_offers.len() < self.max_concurrent_offers {
          guard.pending_offers.push_back(PendingOffer { value, completion: completion.clone() });
        } else {
          completion.complete(Ok(QueueOfferResult::Failure(StreamError::WouldBlock)));
        }
      },
      | OverflowStrategy::DropHead => {
        let _ = guard.values.pop_front();
        guard.values.push_back(value);
        completion.complete(Ok(QueueOfferResult::Enqueued));
      },
      | OverflowStrategy::DropTail => {
        let _ = guard.values.pop_back();
        guard.values.push_back(value);
        completion.complete(Ok(QueueOfferResult::Enqueued));
      },
      | OverflowStrategy::DropBuffer => {
        guard.values.clear();
        guard.values.push_back(value);
        completion.complete(Ok(QueueOfferResult::Enqueued));
      },
      | OverflowStrategy::Fail => {
        self.fail_with_guard(&mut guard, StreamError::BufferOverflow);
        core::mem::drop(guard);
        completion.complete(Ok(QueueOfferResult::Failure(StreamError::BufferOverflow)));
      },
    };
    completion
  }

  fn offer_without_buffer(
    &self,
    guard: &mut SourceQueueWithCompleteState<T>,
    value: T,
    completion: StreamCompletion<QueueOfferResult>,
  ) -> StreamCompletion<QueueOfferResult> {
    if guard.pending_offers.len() < self.max_concurrent_offers {
      guard.pending_offers.push_back(PendingOffer { value, completion: completion.clone() });
      return completion;
    }

    match self.overflow_strategy {
      | OverflowStrategy::Backpressure => {
        completion.complete(Ok(QueueOfferResult::Failure(StreamError::WouldBlock)));
      },
      | OverflowStrategy::DropHead => {
        if let Some(oldest) = guard.pending_offers.pop_front() {
          oldest.completion.complete(Ok(QueueOfferResult::Dropped));
        }
        guard.pending_offers.push_back(PendingOffer { value, completion: completion.clone() });
      },
      | OverflowStrategy::DropTail => {
        completion.complete(Ok(QueueOfferResult::Dropped));
      },
      | OverflowStrategy::DropBuffer => {
        while let Some(pending_offer) = guard.pending_offers.pop_front() {
          pending_offer.completion.complete(Ok(QueueOfferResult::Dropped));
        }
        guard.pending_offers.push_back(PendingOffer { value, completion: completion.clone() });
      },
      | OverflowStrategy::Fail => {
        self.fail_with_guard(guard, StreamError::BufferOverflow);
        completion.complete(Ok(QueueOfferResult::Failure(StreamError::BufferOverflow)));
      },
    };
    completion
  }

  fn fail_with_guard(&self, guard: &mut SourceQueueWithCompleteState<T>, error: StreamError) {
    guard.failure = Some(error.clone());
    guard.closed = true;
    while let Some(pending_offer) = guard.pending_offers.pop_front() {
      pending_offer.completion.complete(Ok(QueueOfferResult::Failure(error.clone())));
    }
    self.completion.complete(Err(error));
  }

  /// Completes the queue and rejects subsequent offers.
  pub fn complete(&self) {
    let should_complete = {
      let mut guard = self.inner.lock();
      guard.closed = true;
      guard.values.is_empty() && guard.pending_offers.is_empty()
    };
    if should_complete {
      self.completion.complete(Ok(StreamDone::new()));
    }
  }

  pub(crate) fn close_for_cancel(&self) {
    let should_complete = {
      let mut guard = self.inner.lock();
      if guard.failure.is_some() {
        return;
      }
      guard.closed = true;
      guard.values.clear();
      while let Some(pending_offer) = guard.pending_offers.pop_front() {
        pending_offer.completion.complete(Ok(QueueOfferResult::QueueClosed));
      }
      true
    };
    if should_complete {
      self.completion.complete(Ok(StreamDone::new()));
    }
  }

  /// Fails the queue and rejects subsequent offers.
  pub fn fail(&self, error: StreamError) {
    let mut guard = self.inner.lock();
    self.fail_with_guard(&mut guard, error);
  }

  /// Returns a handle that can be used to observe stream completion.
  #[must_use]
  pub fn watch_completion(&self) -> StreamCompletion<StreamDone> {
    self.completion.clone()
  }

  /// Returns the configured capacity.
  #[must_use]
  pub const fn capacity(&self) -> usize {
    self.capacity
  }

  /// Returns the configured overflow strategy.
  #[must_use]
  pub const fn overflow_strategy(&self) -> OverflowStrategy {
    self.overflow_strategy
  }

  /// Returns the configured maximum number of pending offers.
  #[must_use]
  pub const fn max_concurrent_offers(&self) -> usize {
    self.max_concurrent_offers
  }

  /// Returns `true` when the queue is closed.
  #[must_use]
  pub fn is_closed(&self) -> bool {
    let guard = self.inner.lock();
    guard.closed
  }

  /// Returns the number of queued elements.
  #[must_use]
  pub fn len(&self) -> usize {
    let guard = self.inner.lock();
    guard.values.len()
  }

  /// Returns `true` when the queue contains no elements.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Polls the next queued element.
  ///
  /// # Errors
  ///
  /// Returns the stored [`StreamError`] if the queue has been failed.
  pub fn poll(&self) -> Result<Option<T>, StreamError> {
    let (value, drained) = {
      let mut guard = self.inner.lock();
      if let Some(error) = &guard.failure {
        return Err(error.clone());
      }
      let value = if self.capacity == 0 {
        match guard.pending_offers.pop_front() {
          | Some(pending_offer) => {
            pending_offer.completion.complete(Ok(QueueOfferResult::Enqueued));
            Some(pending_offer.value)
          },
          | None => None,
        }
      } else {
        let value = guard.values.pop_front();
        while guard.values.len() < self.capacity {
          let Some(pending_offer) = guard.pending_offers.pop_front() else {
            break;
          };
          guard.values.push_back(pending_offer.value);
          pending_offer.completion.complete(Ok(QueueOfferResult::Enqueued));
        }
        value
      };
      let drained = guard.closed && guard.values.is_empty() && guard.pending_offers.is_empty();
      (value, drained)
    };
    if drained {
      self.completion.complete(Ok(StreamDone::new()));
    }
    Ok(value)
  }

  /// Polls the next value and checks drained status atomically under a single
  /// lock acquisition. Avoids TOCTOU races between `poll()` and `is_drained()`.
  pub(crate) fn poll_or_drain(&self) -> Result<Option<T>, StreamError> {
    let (value, drained) = {
      let mut guard = self.inner.lock();
      if let Some(error) = &guard.failure {
        return Err(error.clone());
      }
      let value = if self.capacity == 0 {
        match guard.pending_offers.pop_front() {
          | Some(pending_offer) => {
            pending_offer.completion.complete(Ok(QueueOfferResult::Enqueued));
            Some(pending_offer.value)
          },
          | None => None,
        }
      } else {
        let value = guard.values.pop_front();
        while guard.values.len() < self.capacity {
          let Some(pending_offer) = guard.pending_offers.pop_front() else {
            break;
          };
          guard.values.push_back(pending_offer.value);
          pending_offer.completion.complete(Ok(QueueOfferResult::Enqueued));
        }
        value
      };
      let drained = guard.closed && guard.values.is_empty() && guard.pending_offers.is_empty();
      (value, drained)
    };
    if drained {
      self.completion.complete(Ok(StreamDone::new()));
    }
    // NOTE: capacity > 0 で values が空のとき、pending offers を values に移動した後でも
    // value は None のまま WouldBlock を返す。これは元の poll() と同じ挙動であり、
    // 呼び出し側は次の pull で移動済みの値を取得する。即時返却への変更は今回のスコープ外。
    match value {
      | Some(v) => Ok(Some(v)),
      | None if drained => Ok(None),
      | None => Err(StreamError::WouldBlock),
    }
  }

  /// Returns `true` when the queue is closed and all queued elements were consumed.
  #[must_use]
  pub fn is_drained(&self) -> bool {
    let guard = self.inner.lock();
    guard.closed && guard.values.is_empty() && guard.pending_offers.is_empty()
  }
}
