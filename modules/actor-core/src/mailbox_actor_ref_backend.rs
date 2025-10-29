//! ActorRef backend that enqueues messages into a mailbox instance.

use cellactor_utils_core_rs::{ArcShared, sync::async_mutex_like::SpinAsyncMutex};

use crate::{
  actor_ref_backend::ActorRefBackend, any_message::AnyOwnedMessage, mailbox::Mailbox, pid::Pid, send_error::SendError,
};

/// Backend bridging [`ActorRef`](crate::ActorRef) to a mailbox queue.
pub struct MailboxActorRefBackend {
  pid:     Pid,
  mailbox: ArcShared<SpinAsyncMutex<Mailbox>>,
  system:  bool,
}

impl MailboxActorRefBackend {
  /// Creates a backend targeting the user queue.
  #[must_use]
  pub fn user(pid: Pid, mailbox: ArcShared<SpinAsyncMutex<Mailbox>>) -> Self {
    {
      let mut guard = mailbox.lock();
      guard.bind_pid(pid);
    }
    Self { pid, mailbox, system: false }
  }

  /// Creates a backend targeting the system queue.
  #[must_use]
  pub fn system(pid: Pid, mailbox: ArcShared<SpinAsyncMutex<Mailbox>>) -> Self {
    {
      let mut guard = mailbox.lock();
      guard.bind_pid(pid);
    }
    Self { pid, mailbox, system: true }
  }
}

impl ActorRefBackend for MailboxActorRefBackend {
  fn pid(&self) -> Option<Pid> {
    Some(self.pid)
  }

  fn send(&self, message: AnyOwnedMessage) -> Result<(), SendError> {
    let mut mailbox = self.mailbox.lock();
    if self.system { mailbox.enqueue_system(message) } else { mailbox.enqueue_user(message) }
  }
}
