//! Builtin spin-based runtime lock provider.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex, WeakShared};

use super::{ActorRuntimeLockProvider, DispatcherLockCell, ExecutorLockCell, MailboxLockSet, SenderLockCell};
use crate::core::kernel::{
  actor::{ActorCell, actor_ref::ActorRefSender, messaging::message_invoker::MessageInvokerShared},
  dispatch::{
    dispatcher::{Executor, MessageDispatcher},
    mailbox::MailboxInstrumentation,
  },
};

/// Builtin spin-based runtime lock provider used by default actor-system config.
pub struct BuiltinSpinRuntimeLockProvider;

impl BuiltinSpinRuntimeLockProvider {
  /// Returns a shared builtin provider handle.
  #[must_use]
  pub fn shared() -> ArcShared<dyn ActorRuntimeLockProvider> {
    let provider: ArcShared<dyn ActorRuntimeLockProvider> = ArcShared::new(Self);
    provider
  }
}

impl Default for BuiltinSpinRuntimeLockProvider {
  fn default() -> Self {
    Self
  }
}

impl ActorRuntimeLockProvider for BuiltinSpinRuntimeLockProvider {
  fn new_dispatcher_cell(&self, dispatcher: Box<dyn MessageDispatcher>) -> DispatcherLockCell {
    let read_lock = ArcShared::new(SpinSyncMutex::new(dispatcher));
    let write_lock = read_lock.clone();
    DispatcherLockCell::new(
      move |f| {
        let guard = read_lock.lock();
        f(&guard);
      },
      move |f| {
        let mut guard = write_lock.lock();
        f(&mut guard);
      },
    )
  }

  fn new_executor_cell(&self, executor: Box<dyn Executor>) -> ExecutorLockCell {
    let read_lock = ArcShared::new(SpinSyncMutex::new(executor));
    let write_lock = read_lock.clone();
    ExecutorLockCell::new(
      move |f| {
        let guard = read_lock.lock();
        f(&guard);
      },
      move |f| {
        let mut guard = write_lock.lock();
        f(&mut guard);
      },
    )
  }

  fn new_sender_cell(&self, sender: Box<dyn ActorRefSender>) -> SenderLockCell {
    let read_lock = ArcShared::new(SpinSyncMutex::new(sender));
    let write_lock = read_lock.clone();
    SenderLockCell::new(
      move |f| {
        let guard = read_lock.lock();
        f(&guard);
      },
      move |f| {
        let mut guard = write_lock.lock();
        f(&mut guard);
      },
    )
  }

  fn new_mailbox_lock_set(&self) -> MailboxLockSet {
    let user_queue_lock = ArcShared::new(SpinSyncMutex::new(()));
    let instrumentation = ArcShared::new(SpinSyncMutex::new(None::<MailboxInstrumentation>));
    let invoker = ArcShared::new(SpinSyncMutex::new(None::<MessageInvokerShared>));
    let actor = ArcShared::new(SpinSyncMutex::new(None::<WeakShared<ActorCell>>));

    let instrumentation_read = instrumentation.clone();
    let invoker_read = invoker.clone();
    let actor_read = actor.clone();

    MailboxLockSet::new(
      move |f| {
        let _guard = user_queue_lock.lock();
        f();
      },
      move |f| {
        let guard = instrumentation_read.lock();
        f(&guard);
      },
      move |f| {
        let mut guard = instrumentation.lock();
        f(&mut guard);
      },
      move |f| {
        let guard = invoker_read.lock();
        f(&guard);
      },
      move |f| {
        let mut guard = invoker.lock();
        f(&mut guard);
      },
      move |f| {
        let guard = actor_read.lock();
        f(&guard);
      },
      move |f| {
        let mut guard = actor.lock();
        f(&mut guard);
      },
    )
  }
}
