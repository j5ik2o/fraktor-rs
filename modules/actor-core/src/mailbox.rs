//! Priority mailbox backed by `cellactor-utils-core` queues.

use cellactor_utils_core_rs::{
  collections::queue::QueueError,
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
  v2::collections::queue::{MpscQueue, OfferOutcome, OverflowPolicy as QueueOverflowPolicy, VecRingBackend, VecRingStorage},
};

use crate::{
  any_message::AnyOwnedMessage,
  mailbox_policy::{MailboxPolicy, OverflowPolicy},
  pid::Pid,
  props::MailboxConfig,
  send_error::SendError,
};

/// Mailbox storing system and user message queues with priority dispatch.
pub struct Mailbox {
  policy:            MailboxPolicy,
  throughput_limit:  u32,
  warning_threshold: Option<usize>,
  suspended:         bool,
  pid:               Option<Pid>,
  system:            QueueState,
  user:              QueueState,
}

impl Mailbox {
  /// Creates a new mailbox using the supplied configuration.
  #[must_use]
  pub fn new(config: &MailboxConfig) -> Self {
    let policy = *config.policy();
    let throughput_limit = config.throughput_limit();
    let warning_threshold = config.warning_threshold();

    let (system_state, user_state) = match policy {
      | MailboxPolicy::Unbounded => {
        let initial_capacity = 64;
        let system = QueueState::new(initial_capacity, OverflowPolicy::Grow);
        let user = QueueState::new(initial_capacity, OverflowPolicy::Grow);
        (system, user)
      },
      | MailboxPolicy::Bounded { capacity, overflow } => {
        let (user_capacity, system_capacity) = split_capacity(capacity, config.system_queue_ratio());
        let system = QueueState::new(system_capacity.max(1), overflow);
        let user = QueueState::new(user_capacity.max(1), overflow);
        (system, user)
      },
    };

    Self {
      policy,
      throughput_limit,
      warning_threshold,
      suspended: false,
      pid: None,
      system: system_state,
      user: user_state,
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
  pub fn enqueue_system(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    self.system.offer(message, self.pid)
  }

  /// Enqueues a user message.
  pub fn enqueue_user(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    if self.suspended {
      return Err(SendError::mailbox_suspended(self.pid, message));
    }
    self.user.offer(message, self.pid)
  }

  /// Dequeues the next available message, prioritising system messages.
  #[must_use]
  pub fn dequeue(&self) -> Option<(bool, AnyOwnedMessage)> {
    if let Some(message) = self.system.poll() {
      return Some((true, message));
    }

    if self.suspended {
      return None;
    }

    self.user.poll().map(|message| (false, message))
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
}

/// Queue state wrapper around the utils-core ring buffer backend.
struct QueueState {
  queue:    MpscQueue<AnyOwnedMessage, VecRingBackend<AnyOwnedMessage>>,
  overflow: OverflowPolicy,
}

impl QueueState {
  fn new(initial_capacity: usize, overflow: OverflowPolicy) -> Self {
    let backend_policy = match overflow {
      | OverflowPolicy::Grow => QueueOverflowPolicy::Grow,
      | OverflowPolicy::DropNewest | OverflowPolicy::DropOldest | OverflowPolicy::Block => QueueOverflowPolicy::Block,
    };

    let storage = VecRingStorage::with_capacity(initial_capacity.max(1));
    let backend = VecRingBackend::new_with_storage(storage, backend_policy);
    let shared = ArcShared::new(SpinSyncMutex::new(backend));
    let queue = MpscQueue::new_mpsc(shared);

    Self { queue, overflow }
  }

  fn offer(&self, mut message: AnyOwnedMessage, pid: Option<Pid>) -> Result<(), SendError> {
    loop {
      let clone_for_disconnect = message.clone();
      match self.queue.offer(message) {
        | Ok(OfferOutcome::Enqueued) | Ok(OfferOutcome::GrewTo { .. }) => return Ok(()),
        | Ok(_) => return Ok(()),
        | Err(QueueError::Full(item)) => match self.overflow {
          | OverflowPolicy::Grow => return Err(SendError::mailbox_full(pid, item)),
          | OverflowPolicy::DropNewest => return Err(SendError::mailbox_full(pid, item)),
          | OverflowPolicy::DropOldest => match self.queue.poll() {
            | Ok(_) => {
              message = item;
              continue;
            },
            | Err(_) => return Err(SendError::mailbox_full(pid, item)),
          },
          | OverflowPolicy::Block => return Err(SendError::mailbox_full(pid, item)),
        },
        | Err(QueueError::OfferError(item)) | Err(QueueError::Closed(item)) => {
          return Err(SendError::closed(pid, item));
        },
        | Err(QueueError::AllocError(item)) => return Err(SendError::mailbox_full(pid, item)),
        | Err(QueueError::Disconnected) => return Err(SendError::closed(pid, clone_for_disconnect)),
        | Err(QueueError::WouldBlock) => return Err(SendError::mailbox_full(pid, clone_for_disconnect)),
        | Err(QueueError::Empty) => unreachable!(),
      }
    }
  }

  fn poll(&self) -> Option<AnyOwnedMessage> {
    match self.queue.poll() {
      | Ok(message) => Some(message),
      | Err(QueueError::Empty) => None,
      | Err(QueueError::Disconnected) => None,
      | Err(QueueError::Closed(_)) => None,
      | Err(QueueError::Full(_))
      | Err(QueueError::OfferError(_))
      | Err(QueueError::AllocError(_))
      | Err(QueueError::WouldBlock) => None,
    }
  }

  fn len(&self) -> usize {
    self.queue.len()
  }

  fn capacity(&self) -> Option<usize> {
    match self.overflow {
      | OverflowPolicy::Grow => None,
      | OverflowPolicy::DropNewest | OverflowPolicy::DropOldest | OverflowPolicy::Block => Some(self.queue.capacity()),
    }
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
