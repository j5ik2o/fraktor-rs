//! Simple future-like primitive used by `ask` helpers.

use core::hint::spin_loop;

use cellactor_utils_core_rs::sync::ArcShared;
use spin::Mutex;

use crate::{actor_ref::ActorRef, any_owned_message::AnyOwnedMessage};

/// Shared state backing an [`ActorFuture`].
struct ActorFutureState<T> {
  value:     Option<T>,
  completed: bool,
}

impl<T> ActorFutureState<T> {
  const fn new() -> Self {
    Self { value: None, completed: false }
  }
}

/// Cooperative future used by the runtime to deliver ask responses.
pub struct ActorFuture<T> {
  state: ArcShared<Mutex<ActorFutureState<T>>>,
}

impl<T> ActorFuture<T> {
  /// Creates a pending future.
  #[must_use]
  pub fn pending() -> Self {
    Self { state: ArcShared::new(Mutex::new(ActorFutureState::new())) }
  }

  /// Completes the future; subsequent completions are ignored.
  pub fn complete(&self, value: T) {
    let mut guard = self.state.lock();
    if guard.completed {
      return;
    }
    guard.value = Some(value);
    guard.completed = true;
  }

  /// Returns `true` when a value has been written.
  #[must_use]
  pub fn is_completed(&self) -> bool {
    self.state.lock().completed
  }

  /// Attempts to take the value; returns `None` if not completed yet.
  pub fn try_take(&self) -> Option<T> {
    let mut guard = self.state.lock();
    if guard.completed {
      guard.completed = false;
      guard.value.take()
    } else {
      None
    }
  }

  /// Busy waits until the value is available and then consumes it.
  pub fn wait(self) -> T {
    loop {
      if let Some(value) = self.try_take() {
        return value;
      }
      spin_loop();
    }
  }
}

impl<T> Default for ActorFuture<T> {
  fn default() -> Self {
    Self::pending()
  }
}

impl<T> Clone for ActorFuture<T> {
  fn clone(&self) -> Self {
    Self { state: self.state.clone() }
  }
}

impl ActorFuture<AnyOwnedMessage> {
  #[must_use]
  pub(crate) fn reply_handle(&self) -> ActorRef {
    ActorRef::for_future(self.clone())
  }
}
