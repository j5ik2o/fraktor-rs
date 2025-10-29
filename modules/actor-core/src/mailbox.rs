//! Mailbox implementation handling System/User queues and overflow policies.

use alloc::collections::VecDeque;

use crate::{
  any_owned_message::AnyOwnedMessage,
  mailbox_policy::MailboxPolicy,
  props::{MailboxCapacity, MailboxConfig},
};

/// Result of enqueuing a message into the mailbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxEnqueue {
  /// Message stored without side effects.
  Enqueued,
  /// Message stored after dropping the oldest entry.
  DroppedOldest,
  /// Message dropped because it was the newest entry.
  DroppedNewest,
}

/// Errors that can occur while interacting with the mailbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxError {
  /// Mailbox is at capacity and the policy requires blocking.
  WouldBlock,
  /// Mailbox is suspended for user traffic.
  Suspended,
}

/// Mailbox storing system-priority and user messages separately.
pub struct Mailbox {
  system_queue: VecDeque<AnyOwnedMessage>,
  user_queue:   VecDeque<AnyOwnedMessage>,
  capacity:     MailboxCapacity,
  policy:       MailboxPolicy,
  suspended:    bool,
}

impl Mailbox {
  /// Creates a mailbox based on the provided configuration.
  #[must_use]
  pub fn new(config: MailboxConfig) -> Self {
    let (capacity, policy) = (config.capacity(), config.policy());
    Self { system_queue: VecDeque::new(), user_queue: VecDeque::new(), capacity, policy, suspended: false }
  }

  fn total_len(&self) -> usize {
    self.system_queue.len() + self.user_queue.len()
  }

  fn max_capacity(&self) -> Option<usize> {
    match self.capacity {
      | MailboxCapacity::Bounded(limit) => Some(limit.max(1)),
      | MailboxCapacity::Unbounded => None,
    }
  }

  fn is_full(&self) -> bool {
    self.max_capacity().map(|limit| self.total_len() >= limit).unwrap_or(false)
  }

  /// Adds a System message to the queue. System messages bypass suspension rules.
  pub fn enqueue_system(&mut self, message: AnyOwnedMessage) -> MailboxEnqueue {
    self.enqueue_internal(message, true).unwrap_or(MailboxEnqueue::Enqueued)
  }

  /// Adds a User message to the queue, respecting suspension and overflow policy.
  pub fn enqueue_user(&mut self, message: AnyOwnedMessage) -> Result<MailboxEnqueue, MailboxError> {
    if self.suspended {
      return Err(MailboxError::Suspended);
    }
    self.enqueue_internal(message, false).ok_or(MailboxError::WouldBlock)
  }

  fn enqueue_internal(&mut self, message: AnyOwnedMessage, system: bool) -> Option<MailboxEnqueue> {
    if !system && self.is_full() {
      return match self.policy {
        | MailboxPolicy::DropNewest => Some(MailboxEnqueue::DroppedNewest),
        | MailboxPolicy::DropOldest | MailboxPolicy::Default => {
          let _ = self.user_queue.pop_front();
          self.user_queue.push_back(message);
          Some(MailboxEnqueue::DroppedOldest)
        },
        | MailboxPolicy::Grow | MailboxPolicy::Block => None,
      };
    }

    if system {
      self.system_queue.push_back(message);
    } else {
      if self.is_full() {
        let current_len = self.total_len();
        match self.policy {
          | MailboxPolicy::DropNewest => return Some(MailboxEnqueue::DroppedNewest),
          | MailboxPolicy::DropOldest | MailboxPolicy::Default => {
            let _ = self.user_queue.pop_front();
          },
          | MailboxPolicy::Grow => {
            if let MailboxCapacity::Bounded(ref mut limit) = self.capacity {
              let desired = current_len.saturating_add(1);
              *limit = (*limit).saturating_mul(2).max(desired);
            }
          },
          | MailboxPolicy::Block => return None,
        }
      }
      self.user_queue.push_back(message);
    }

    Some(MailboxEnqueue::Enqueued)
  }

  /// Retrieves the next message, prioritising System queue.
  #[must_use]
  pub fn dequeue(&mut self) -> Option<AnyOwnedMessage> {
    if let Some(message) = self.system_queue.pop_front() {
      return Some(message);
    }
    self.user_queue.pop_front()
  }

  /// Suspends user message processing.
  pub fn suspend(&mut self) {
    self.suspended = true;
  }

  /// Resumes user message processing.
  pub fn resume(&mut self) {
    self.suspended = false;
  }

  /// Returns whether the mailbox has pending messages.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.system_queue.is_empty() && self.user_queue.is_empty()
  }

  /// Returns the number of pending system messages.
  #[must_use]
  pub fn system_len(&self) -> usize {
    self.system_queue.len()
  }

  /// Returns the number of pending user messages.
  #[must_use]
  pub fn user_len(&self) -> usize {
    self.user_queue.len()
  }

  /// Clears the mailbox.
  pub fn clear(&mut self) {
    self.system_queue.clear();
    self.user_queue.clear();
  }
}
