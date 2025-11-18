//! Priority queue separating system and user messages.

use alloc::collections::VecDeque;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

/// Differentiates system vs user envelopes.
pub enum EnvelopePriority {
  /// System messages have higher priority.
  System,
  /// User messages are processed after system queues drain.
  User,
}

/// Queues outbound envelopes while respecting system priority and backpressure signals.
pub struct OutboundQueue<TB: RuntimeToolbox + 'static, T> {
  system:      VecDeque<T>,
  user:        VecDeque<T>,
  user_paused: bool,
  _marker:     core::marker::PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static, T> OutboundQueue<TB, T> {
  /// Creates an empty queue.
  #[must_use]
  pub const fn new() -> Self {
    Self { system: VecDeque::new(), user: VecDeque::new(), user_paused: false, _marker: core::marker::PhantomData }
  }

  /// Enqueues the payload using the provided priority classifier.
  pub fn push<F>(&mut self, item: T, classify: F)
  where
    F: Fn(&T) -> EnvelopePriority,
  {
    match classify(&item) {
      | EnvelopePriority::System => self.system.push_back(item),
      | EnvelopePriority::User => self.user.push_back(item),
    }
  }

  /// Pops the next element, draining system queue before user queue.
  #[must_use]
  pub fn pop(&mut self) -> Option<T> {
    if let Some(item) = self.system.pop_front() {
      return Some(item);
    }
    if self.user_paused {
      None
    } else {
      self.user.pop_front()
    }
  }

  /// Pauses draining of user messages while honoring system priority.
  pub fn pause_user(&mut self) {
    self.user_paused = true;
  }

  /// Resumes draining of user messages.
  pub fn resume_user(&mut self) {
    self.user_paused = false;
  }
}
