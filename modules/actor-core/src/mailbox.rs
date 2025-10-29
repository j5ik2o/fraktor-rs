//! Priority mailbox backed by utils-core asynchronous queues.

use alloc::vec::Vec;
use core::{
  future::Future,
  pin::pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use cellactor_utils_core_rs::{
  Shared,
  collections::queue::QueueError,
  sync::{ArcShared, async_mutex_like::SpinAsyncMutex},
  v2::collections::queue::{
    AsyncMpscQueue, AsyncQueueBackend, OfferOutcome, OverflowPolicy as QueueOverflowPolicy, SyncAdapterQueueBackend,
    VecRingBackend, VecRingStorage,
  },
};

use crate::{
  any_message::AnyOwnedMessage,
  mailbox_policy::{MailboxPolicy, OverflowPolicy},
  pid::Pid,
  props::MailboxConfig,
  send_error::SendError,
  system_message::SystemMessage,
};

type MailboxBackend<T> = SyncAdapterQueueBackend<T, VecRingBackend<T>>;
type MailboxMutex<T> = SpinAsyncMutex<MailboxBackend<T>>;
type MailboxShared<T> = ArcShared<MailboxMutex<T>>;
type MailboxAsyncQueue<T> = AsyncMpscQueue<T, MailboxBackend<T>, MailboxMutex<T>>;

/// Mailbox storing system and user message queues with priority dispatch.
pub struct Mailbox {
  throughput_limit:  u32,
  warning_threshold: Option<usize>,
  suspended:         bool,
  pid:               Option<Pid>,
  system:            QueueState<SystemMessage>,
  user:              QueueState<AnyOwnedMessage>,
  dropped_system:    Vec<SystemMessage>,
  dropped_user:      Vec<AnyOwnedMessage>,
}

impl Mailbox {
  /// Creates a new mailbox using the supplied configuration.
  #[must_use]
  pub fn new(config: &MailboxConfig) -> Self {
    let policy = *config.policy();
    let throughput_limit = config.throughput_limit();
    let warning_threshold = config.warning_threshold();

    let (user_capacity, system_capacity, user_overflow) = match policy {
      | MailboxPolicy::Unbounded => (64, 16, OverflowPolicy::Grow),
      | MailboxPolicy::Bounded { capacity, overflow } => {
        let (user, system) = split_capacity(capacity, config.system_queue_ratio());
        (user.max(1), system.max(1), overflow)
      },
    };

    let system = QueueState::new(system_capacity, OverflowPolicy::DropNewest);
    let user = QueueState::new(user_capacity, user_overflow);

    Self {
      throughput_limit,
      warning_threshold,
      suspended: false,
      pid: None,
      system,
      user,
      dropped_system: Vec::new(),
      dropped_user: Vec::new(),
    }
  }

  /// Associates the mailbox with a PID.
  pub fn bind_pid(&mut self, pid: Pid) {
    self.pid = Some(pid);
  }

  /// Returns the PID associated with this mailbox when available.
  #[must_use]
  pub const fn pid(&self) -> Option<Pid> {
    self.pid
  }

  /// Returns the configured throughput limit.
  #[must_use]
  pub const fn throughput_limit(&self) -> u32 {
    self.throughput_limit
  }

  /// Returns the warning threshold if configured.
  #[must_use]
  pub const fn warning_threshold(&self) -> Option<usize> {
    self.warning_threshold
  }

  /// Returns the current number of enqueued messages.
  #[must_use]
  pub fn len(&self) -> usize {
    self.system.len() + self.user.len()
  }

  /// Indicates whether both queues are empty.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.system.len() == 0 && self.user.len() == 0
  }

  /// Suspends user-level message processing.
  pub fn suspend(&mut self) {
    self.suspended = true;
  }

  /// Resumes user-level message processing.
  pub fn resume(&mut self) {
    self.suspended = false;
  }

  /// Returns whether the mailbox is suspended for user messages.
  #[must_use]
  pub const fn is_suspended(&self) -> bool {
    self.suspended
  }

  /// Enqueues a system message.
  pub fn enqueue_system(&mut self, message: SystemMessage) -> Result<(), SendError<SystemMessage>> {
    match self.system.offer(message, self.pid) {
      | Ok(Some(dropped)) => {
        self.dropped_system.push(dropped);
        Ok(())
      },
      | Ok(None) => Ok(()),
      | Err(err) => Err(err),
    }
  }

  /// Enqueues a user message.
  pub fn enqueue_user(&mut self, message: AnyOwnedMessage) -> Result<(), SendError<AnyOwnedMessage>> {
    if self.suspended {
      return Err(SendError::mailbox_suspended(self.pid, message));
    }

    match self.user.offer(message, self.pid) {
      | Ok(Some(dropped)) => {
        self.dropped_user.push(dropped);
        Ok(())
      },
      | Ok(None) => Ok(()),
      | Err(err) => Err(err),
    }
  }

  /// Dequeues the next available message, prioritising system messages.
  #[must_use]
  pub fn dequeue(&mut self) -> Option<DequeuedMessage> {
    if let Some(message) = self.system.poll() {
      return Some(DequeuedMessage::System(message));
    }

    if self.suspended {
      return None;
    }

    self.user.poll().map(DequeuedMessage::User)
  }

  /// Drains the list of recently dropped user messages (DropOldest policy).
  pub fn take_dropped_user(&mut self) -> Vec<AnyOwnedMessage> {
    core::mem::take(&mut self.dropped_user)
  }

  /// Drains the list of recently dropped system messages.
  pub fn take_dropped_system(&mut self) -> Vec<SystemMessage> {
    core::mem::take(&mut self.dropped_system)
  }

  /// Returns queue occupancy metrics for diagnostics.
  #[must_use]
  pub fn occupancy(&self) -> MailboxOccupancy {
    MailboxOccupancy {
      system_len:      self.system.len(),
      system_capacity: self.system.capacity(),
      user_len:        self.user.len(),
      user_capacity:   self.user.capacity(),
    }
  }

  /// Provides an async handle to the user queue for dispatcher integration.
  #[must_use]
  pub fn async_user_queue(&self) -> MailboxAsyncQueue<AnyOwnedMessage> {
    self.user.async_handle()
  }

  /// Provides an async handle to the system queue for dispatcher integration.
  #[must_use]
  pub fn async_system_queue(&self) -> MailboxAsyncQueue<SystemMessage> {
    self.system.async_handle()
  }
}

/// Snapshot of mailbox occupancy.
pub struct MailboxOccupancy {
  /// Number of messages in the system queue.
  pub system_len:      usize,
  /// Capacity of the system queue when bounded.
  pub system_capacity: Option<usize>,
  /// Number of messages in the user queue.
  pub user_len:        usize,
  /// Capacity of the user queue when bounded.
  pub user_capacity:   Option<usize>,
}

/// Result of dequeuing a message.
pub enum DequeuedMessage {
  /// System-level control message.
  System(SystemMessage),
  /// User-level message.
  User(AnyOwnedMessage),
}

struct QueueState<T> {
  shared:   MailboxShared<T>,
  queue:    MailboxAsyncQueue<T>,
  overflow: OverflowPolicy,
}

impl<T> QueueState<T>
where
  T: Clone,
{
  fn new(initial_capacity: usize, overflow: OverflowPolicy) -> Self {
    let storage = VecRingStorage::with_capacity(initial_capacity.max(1));
    let queue_policy = match overflow {
      | OverflowPolicy::Grow => QueueOverflowPolicy::Grow,
      | OverflowPolicy::DropNewest | OverflowPolicy::DropOldest | OverflowPolicy::Block => QueueOverflowPolicy::Block,
    };
    let backend = VecRingBackend::new_with_storage(storage, queue_policy);
    let adapter = SyncAdapterQueueBackend::new(backend);
    let mutex = SpinAsyncMutex::new(adapter);
    let shared = ArcShared::new(mutex);
    let queue = AsyncMpscQueue::new_mpsc(shared.clone());

    Self { shared, queue, overflow }
  }

  fn offer(&self, message: T, pid: Option<Pid>) -> Result<Option<T>, SendError<T>> {
    self.shared.with_ref(|mutex: &MailboxMutex<T>| {
      let mut guard = mutex.lock();
      let adapter: &mut MailboxBackend<T> = &mut *guard;

      let len = adapter.len();
      let capacity = adapter.capacity();

      if len >= capacity {
        match self.overflow {
          | OverflowPolicy::DropNewest => return Err(SendError::mailbox_full(pid, message)),
          | OverflowPolicy::DropOldest => {
            let dropped = Self::poll_immediate(adapter).map_err(|err| convert_queue_error(pid, err, None))?;
            let fallback_new = dropped_new_fallback(&message);
            Self::offer_immediate(adapter, message).map_err(|err| convert_queue_error(pid, err, fallback_new))?;
            return Ok(Some(dropped));
          },
          | OverflowPolicy::Block => return Err(SendError::mailbox_full(pid, message)),
          | OverflowPolicy::Grow => {
            let fallback = Some(message.clone());
            Self::offer_immediate(adapter, message).map_err(|err| convert_queue_error(pid, err, fallback))?;
            return Ok(None);
          },
        }
      }

      let fallback = Some(message.clone());
      Self::offer_immediate(adapter, message).map_err(|err| convert_queue_error(pid, err, fallback))?;
      Ok(None)
    })
  }

  fn poll(&mut self) -> Option<T> {
    self.shared.with_ref(|mutex: &MailboxMutex<T>| {
      let mut guard = mutex.lock();
      let adapter: &mut MailboxBackend<T> = &mut *guard;
      match Self::poll_immediate(adapter) {
        | Ok(value) => Some(value),
        | Err(QueueError::Empty) | Err(QueueError::Disconnected) => None,
        | Err(QueueError::Closed(_))
        | Err(QueueError::Full(_))
        | Err(QueueError::OfferError(_))
        | Err(QueueError::AllocError(_))
        | Err(QueueError::WouldBlock) => None,
      }
    })
  }

  fn len(&self) -> usize {
    self.shared.with_ref(|mutex: &MailboxMutex<T>| {
      let guard = mutex.lock();
      (*guard).len()
    })
  }

  fn capacity(&self) -> Option<usize> {
    self.shared.with_ref(|mutex: &MailboxMutex<T>| {
      let guard = mutex.lock();
      match self.overflow {
        | OverflowPolicy::Grow => None,
        | OverflowPolicy::DropNewest | OverflowPolicy::DropOldest | OverflowPolicy::Block => Some((*guard).capacity()),
      }
    })
  }

  fn async_handle(&self) -> MailboxAsyncQueue<T> {
    self.queue.clone()
  }

  fn offer_immediate(adapter: &mut MailboxBackend<T>, message: T) -> Result<OfferOutcome, QueueError<T>> {
    block_on_ready(adapter.offer(message))
  }

  fn poll_immediate(adapter: &mut MailboxBackend<T>) -> Result<T, QueueError<T>> {
    block_on_ready(adapter.poll())
  }
}

fn convert_queue_error<T>(pid: Option<Pid>, error: QueueError<T>, fallback: Option<T>) -> SendError<T>
where
  T: Clone, {
  match error {
    | QueueError::Full(item) | QueueError::OfferError(item) | QueueError::AllocError(item) => {
      SendError::mailbox_full(pid, item)
    },
    | QueueError::Closed(item) => SendError::closed(pid, item),
    | QueueError::Disconnected => {
      let message = fallback.expect("disconnected queue requires cloned message");
      SendError::closed(pid, message)
    },
    | QueueError::Empty | QueueError::WouldBlock => {
      let message = fallback.expect("empty queue requires cloned message");
      SendError::mailbox_full(pid, message)
    },
  }
}

fn dropped_new_fallback<T>(message: &T) -> Option<T>
where
  T: Clone, {
  Some(message.clone())
}

fn split_capacity(total: usize, system_ratio: f32) -> (usize, usize) {
  if total == 0 {
    return (0, 0);
  }

  let clamped = system_ratio.clamp(0.0, 1.0);
  let mut system = ((total as f32) * clamped + 0.5) as usize;
  if system > total {
    system = total;
  }

  let user = total.saturating_sub(system);
  (user, system)
}

fn block_on_ready<F>(future: F) -> F::Output
where
  F: Future, {
  let waker = unsafe { noop_waker() };
  let mut context = Context::from_waker(&waker);
  let mut future = pin!(future);

  match future.as_mut().poll(&mut context) {
    | Poll::Ready(value) => value,
    | Poll::Pending => panic!("mailbox operations should not yield"),
  }
}

unsafe fn noop_waker() -> Waker {
  fn no_op(_: *const ()) {}

  fn clone(_: *const ()) -> RawWaker {
    RawWaker::new(core::ptr::null(), &VTABLE)
  }

  static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
  let raw = RawWaker::new(core::ptr::null(), &VTABLE);
  unsafe { Waker::from_raw(raw) }
}
