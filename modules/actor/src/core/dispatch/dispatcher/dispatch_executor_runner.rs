//! Runner that serializes dispatch executor calls to avoid deadlock on re-entry.

use alloc::boxed::Box;
use core::sync::atomic::Ordering;

use fraktor_utils_rs::core::{
  collections::queue::{OverflowPolicy, QueueError, SyncFifoQueue, backend::VecDequeBackend},
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::sync_mutex_like::SyncMutexLike,
};
use portable_atomic::AtomicBool;

use super::{
  dispatch_error::DispatchError, dispatch_executor::DispatchExecutor, dispatch_shared::DispatchSharedGeneric,
};

#[cfg(test)]
mod tests;

/// Type alias for the task queue used by [`DispatchExecutorRunner`].
type TaskQueue<TB> = SyncFifoQueue<DispatchSharedGeneric<TB>, VecDequeBackend<DispatchSharedGeneric<TB>>>;

/// Serializing runner for [`DispatchExecutor`] that avoids deadlock on re-entry.
///
/// When `submit` is called during an ongoing execution (re-entry), the task is
/// queued instead of blocking. Only one thread drains the queue at a time.
pub struct DispatchExecutorRunner<TB: RuntimeToolbox + 'static> {
  executor: ToolboxMutex<Box<dyn DispatchExecutor<TB>>, TB>,
  queue:    ToolboxMutex<TaskQueue<TB>, TB>,
  running:  AtomicBool,
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for DispatchExecutorRunner<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for DispatchExecutorRunner<TB> {}

/// Default initial capacity for the task queue.
const DEFAULT_QUEUE_CAPACITY: usize = 16;

/// Converts a [`QueueError`] into a [`DispatchError`].
const fn queue_error_to_dispatch_error<T>(err: &QueueError<T>) -> DispatchError {
  match err {
    | QueueError::Closed(_) | QueueError::Disconnected => DispatchError::ExecutorUnavailable,
    | QueueError::Full(_)
    | QueueError::AllocError(_)
    | QueueError::Empty
    | QueueError::OfferError(_)
    | QueueError::WouldBlock
    | QueueError::TimedOut(_) => DispatchError::RejectedExecution,
  }
}

impl<TB: RuntimeToolbox + 'static> DispatchExecutorRunner<TB> {
  /// Creates a new runner wrapping the given executor.
  #[must_use]
  pub fn new(executor: Box<dyn DispatchExecutor<TB>>) -> Self {
    let backend = VecDequeBackend::with_capacity(DEFAULT_QUEUE_CAPACITY, OverflowPolicy::Grow);
    let queue = SyncFifoQueue::new(backend);
    Self {
      executor: <TB::MutexFamily as SyncMutexFamily>::create(executor),
      queue:    <TB::MutexFamily as SyncMutexFamily>::create(queue),
      running:  AtomicBool::new(false),
    }
  }

  /// Submits a task for execution.
  ///
  /// If no execution is in progress, the caller becomes the drain owner and
  /// executes all queued tasks. If an execution is already in progress (re-entry),
  /// the task is queued and returns immediately without blocking.
  ///
  /// # Errors
  ///
  /// Returns [`DispatchError`] if the underlying executor rejects execution.
  pub fn submit(&self, task: DispatchSharedGeneric<TB>) -> Result<(), DispatchError> {
    // Push task to queue
    self.queue.lock().offer(task).map_err(|e| queue_error_to_dispatch_error(&e))?;

    // Try to become the drain owner
    if self.running.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
      // Another thread is already draining; our task will be picked up by them
      return Ok(());
    }

    // We are the drain owner - process all queued tasks
    let mut result = self.drain_queue();

    // Release drain ownership
    self.running.store(false, Ordering::Release);

    // Check if someone enqueued while we were releasing
    // This prevents lost wakeups and also propagates errors from subsequent drains
    while result.is_ok() && !self.queue.lock().is_empty() {
      if self.running.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
        break;
      }
      let next = self.drain_queue();
      if next.is_err() {
        result = next;
      }
      self.running.store(false, Ordering::Release);
    }

    result
  }

  fn drain_queue(&self) -> Result<(), DispatchError> {
    loop {
      // Pop a task from the queue
      let task = match self.queue.lock().poll() {
        | Ok(task) => task,
        | Err(QueueError::Empty) => break,
        | Err(ref err) => return Err(queue_error_to_dispatch_error(err)),
      };

      // Execute the task with exclusive access to the executor
      let mut guard = self.executor.lock();
      let task_clone = task.clone();
      match guard.execute(task) {
        | Ok(()) => {},
        | Err(error) => {
          // 失敗したタスクをキュー末尾に戻してリトライ可能にする
          // (SyncQueueにはpush_frontがないため末尾に追加)
          // キューへの再追加が失敗した場合はキューエラーを返し、成功したら実行エラーを返す
          if let Err(ref queue_err) = self.queue.lock().offer(task_clone) {
            return Err(queue_error_to_dispatch_error(queue_err));
          }
          return Err(error);
        },
      }
      // Lock is released here before processing next task
    }

    Ok(())
  }

  /// Returns `true` if the underlying executor supports blocking operations.
  pub fn supports_blocking(&self) -> bool {
    self.executor.lock().supports_blocking()
  }
}
