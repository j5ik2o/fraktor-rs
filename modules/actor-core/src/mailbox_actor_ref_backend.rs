//! ActorRef backend that enqueues messages into a mailbox instance.

use cellactor_utils_core_rs::{ArcShared, Shared, sync::async_mutex_like::SpinAsyncMutex};

use crate::{
  actor_ref_backend::ActorRefBackend, any_message::AnyOwnedMessage, mailbox::Mailbox, pid::Pid, send_error::SendError,
};

/// Backend bridging [`ActorRef`](crate::ActorRef) to a mailbox queue.
pub struct MailboxActorRefBackend {
  pid:     Pid,
  mailbox: ArcShared<SpinAsyncMutex<Mailbox>>,
}

impl MailboxActorRefBackend {
  /// Creates a backend targeting the user queue.
  #[must_use]
  pub fn user(pid: Pid, mailbox: ArcShared<SpinAsyncMutex<Mailbox>>) -> Self {
    mailbox.with_ref(|mutex: &SpinAsyncMutex<Mailbox>| {
      let mut guard = mutex.lock();
      guard.bind_pid(pid);
    });
    Self { pid, mailbox }
  }
}

impl ActorRefBackend for MailboxActorRefBackend {
  fn pid(&self) -> Option<Pid> {
    Some(self.pid)
  }

  fn send(&self, message: AnyOwnedMessage) -> Result<(), SendError<AnyOwnedMessage>> {
    self.mailbox.with_ref(|mutex: &SpinAsyncMutex<Mailbox>| {
      let mut guard = mutex.lock();
      guard.enqueue_user(message)
    })
  }
}
