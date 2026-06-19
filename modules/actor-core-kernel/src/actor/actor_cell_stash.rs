//! Actor cell stash facet for actor cells.

use alloc::collections::VecDeque;
use core::mem;

use crate::actor::{
  ActorCell, STASH_OVERFLOW_REASON, STASH_REQUIRES_DEQUE_REASON, error::ActorError, messaging::AnyMessage,
};

impl ActorCell {
  /// Stashes a user message with an explicit stash capacity limit.
  ///
  /// # Errors
  ///
  /// Returns an overflow error when the stash already reached `max_messages`.
  pub(crate) fn stash_message_with_limit(&self, message: AnyMessage, max_messages: usize) -> Result<(), ActorError> {
    if self.mailbox().user_deque().is_none() {
      return Err(ActorError::recoverable(STASH_REQUIRES_DEQUE_REASON));
    }
    self.state.with_write(|state| {
      if state.stashed_messages.len() >= max_messages {
        return Err(ActorError::recoverable(STASH_OVERFLOW_REASON));
      }
      state.stashed_messages.push_back(message);
      Ok(())
    })
  }

  /// Returns the number of messages currently held in the stash.
  #[must_use]
  pub(crate) fn stashed_message_len(&self) -> usize {
    self.state.with_read(|state| state.stashed_messages.len())
  }

  /// Applies a read-only closure to the current stashed messages.
  pub(crate) fn with_stashed_messages<R>(&self, f: impl FnOnce(&VecDeque<AnyMessage>) -> R) -> R {
    self.state.with_read(|state| f(&state.stashed_messages))
  }

  /// Removes all currently stashed messages and returns how many were dropped.
  #[must_use]
  pub(crate) fn clear_stashed_messages(&self) -> usize {
    self.state.with_write(|state| {
      let count = state.stashed_messages.len();
      state.stashed_messages.clear();
      count
    })
  }

  /// Re-enqueues the oldest stashed user message back to this actor mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when mailbox enqueue fails. Remaining messages stay stashed.
  pub(crate) fn unstash_message(&self) -> Result<usize, ActorError> {
    if self.stashed_message_len() == 0 {
      return Ok(0);
    }

    let mailbox = self.mailbox();
    let Some(user_deque) = mailbox.user_deque() else {
      return Err(ActorError::recoverable(STASH_REQUIRES_DEQUE_REASON));
    };

    let message = self.state.with_write(|state| state.stashed_messages.pop_front());

    let Some(message) = message else {
      return Ok(0);
    };

    let mut pending = VecDeque::new();
    pending.push_back(message);

    if let Err(error) = mailbox.prepend_user_messages_deque(user_deque, &pending) {
      self.state.with_write(|state| {
        if let Some(message) = pending.pop_front() {
          state.stashed_messages.push_front(message);
        }
      });
      return Err(ActorError::from_send_error(&error));
    }

    let _scheduled = self.new_dispatcher.register_for_execution(&mailbox, true, false);

    Ok(1)
  }

  /// Re-enqueues all stashed user messages back to this actor mailbox.
  ///
  /// # Errors
  ///
  /// Returns an error when mailbox enqueue fails. Remaining messages stay stashed.
  pub(crate) fn unstash_messages(&self) -> Result<usize, ActorError> {
    if self.stashed_message_len() == 0 {
      return Ok(0);
    }

    let mailbox = self.mailbox();
    let Some(user_deque) = mailbox.user_deque() else {
      return Err(ActorError::recoverable(STASH_REQUIRES_DEQUE_REASON));
    };

    let pending = self.state.with_write(|state| mem::take(&mut state.stashed_messages));

    if pending.is_empty() {
      return Ok(0);
    }

    if let Err(error) = mailbox.prepend_user_messages_deque(user_deque, &pending) {
      self.state.with_write(|state| state.stashed_messages = pending);
      return Err(ActorError::from_send_error(&error));
    }

    let _scheduled = self.new_dispatcher.register_for_execution(&mailbox, true, false);

    Ok(pending.len())
  }

  /// Re-enqueues up to `limit` stashed messages after applying `wrap`.
  ///
  /// # Errors
  ///
  /// Returns an error when message conversion or mailbox enqueue fails.
  pub(crate) fn unstash_messages_with_limit<F>(&self, limit: usize, mut wrap: F) -> Result<usize, ActorError>
  where
    F: FnMut(AnyMessage) -> Result<AnyMessage, ActorError>, {
    if limit == 0 {
      return Ok(0);
    }

    if self.stashed_message_len() == 0 {
      return Ok(0);
    }

    let mailbox = self.mailbox();
    let Some(user_deque) = mailbox.user_deque() else {
      return Err(ActorError::recoverable(STASH_REQUIRES_DEQUE_REASON));
    };

    let original_messages = self.state.with_write(|state| {
      let take_count = limit.min(state.stashed_messages.len());
      let mut messages = VecDeque::with_capacity(take_count);
      for _ in 0..take_count {
        if let Some(message) = state.stashed_messages.pop_front() {
          messages.push_back(message);
        }
      }
      messages
    });

    if original_messages.is_empty() {
      return Ok(0);
    }

    let mut wrapped_messages = VecDeque::with_capacity(original_messages.len());
    for message in original_messages.iter().cloned() {
      match wrap(message) {
        | Ok(wrapped) => wrapped_messages.push_back(wrapped),
        | Err(error) => {
          self.restore_stashed_messages(original_messages);
          return Err(error);
        },
      }
    }

    if let Err(error) = mailbox.prepend_user_messages_deque(user_deque, &wrapped_messages) {
      self.restore_stashed_messages(original_messages);
      return Err(ActorError::from_send_error(&error));
    }

    let _scheduled = self.new_dispatcher.register_for_execution(&mailbox, true, false);

    Ok(wrapped_messages.len())
  }

  fn restore_stashed_messages(&self, mut messages: VecDeque<AnyMessage>) {
    self.state.with_write(|state| {
      while let Some(message) = messages.pop_back() {
        state.stashed_messages.push_front(message);
      }
    });
  }

  pub(super) fn drop_stash_messages(&self) {
    self.state.with_write(|state| state.stashed_messages.clear());
  }
}
