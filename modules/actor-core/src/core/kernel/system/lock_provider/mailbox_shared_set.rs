//! Mailbox lock bundle materialized by an [`ActorLockProvider`](super::ActorLockProvider).

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex, WeakShared};

use crate::core::kernel::{
  actor::{ActorCell, messaging::message_invoker::MessageInvokerShared},
  dispatch::mailbox::MailboxInstrumentation,
  system::lock_provider::{DebugSpinLock, DebugSpinLockGuard},
};

/// Lock bundle used by mailbox hot-path state.
#[derive(Clone)]
pub struct MailboxSharedSet {
  user_queue_lock: MailboxLocked<()>,
  instrumentation: MailboxLocked<Option<MailboxInstrumentation>>,
  invoker:         MailboxLocked<Option<MessageInvokerShared>>,
  actor:           MailboxLocked<Option<WeakShared<ActorCell>>>,
}

impl MailboxSharedSet {
  pub(crate) fn builtin() -> Self {
    Self {
      user_queue_lock: MailboxLocked::builtin(()),
      instrumentation: MailboxLocked::builtin(None),
      invoker:         MailboxLocked::builtin(None),
      actor:           MailboxLocked::builtin(None),
    }
  }

  pub(crate) fn debug() -> Self {
    Self {
      user_queue_lock: MailboxLocked::debug((), "mailbox.user_queue_lock"),
      instrumentation: MailboxLocked::debug(None, "mailbox.instrumentation"),
      invoker:         MailboxLocked::debug(None, "mailbox.invoker"),
      actor:           MailboxLocked::debug(None, "mailbox.actor"),
    }
  }

  pub(crate) fn user_queue_lock(&self) -> MailboxLocked<()> {
    self.user_queue_lock.clone()
  }

  pub(crate) fn instrumentation(&self) -> MailboxLocked<Option<MailboxInstrumentation>> {
    self.instrumentation.clone()
  }

  pub(crate) fn invoker(&self) -> MailboxLocked<Option<MessageInvokerShared>> {
    self.invoker.clone()
  }

  pub(crate) fn actor(&self) -> MailboxLocked<Option<WeakShared<ActorCell>>> {
    self.actor.clone()
  }
}

pub(crate) enum MailboxLockGuard<'a, T> {
  Builtin(spin::MutexGuard<'a, T>),
  Debug(DebugSpinLockGuard<'a, T>),
}

impl<T> core::ops::Deref for MailboxLockGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    match self {
      | Self::Builtin(guard) => guard,
      | Self::Debug(guard) => guard,
    }
  }
}

impl<T> core::ops::DerefMut for MailboxLockGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    match self {
      | Self::Builtin(guard) => guard,
      | Self::Debug(guard) => guard,
    }
  }
}

pub(crate) enum MailboxLocked<T> {
  Builtin(ArcShared<RuntimeMutex<T>>),
  Debug(ArcShared<DebugSpinLock<T>>),
}

impl<T> MailboxLocked<T> {
  fn builtin(value: T) -> Self {
    Self::Builtin(ArcShared::new(RuntimeMutex::new(value)))
  }

  fn debug(value: T, label: &'static str) -> Self {
    Self::Debug(ArcShared::new(DebugSpinLock::new(value, label)))
  }

  pub(crate) fn lock(&self) -> MailboxLockGuard<'_, T> {
    match self {
      | Self::Builtin(inner) => MailboxLockGuard::Builtin(inner.lock()),
      | Self::Debug(inner) => MailboxLockGuard::Debug(inner.lock()),
    }
  }
}

impl<T> Clone for MailboxLocked<T> {
  fn clone(&self) -> Self {
    match self {
      | Self::Builtin(inner) => Self::Builtin(inner.clone()),
      | Self::Debug(inner) => Self::Debug(inner.clone()),
    }
  }
}
