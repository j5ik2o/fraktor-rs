//! Actions derived from journal responses.

use alloc::vec::Vec;

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  eventsourced::Eventsourced, journal_error::JournalError, pending_handler_invocation::PendingHandlerInvocation,
  persistence_error::PersistenceError, persistent_repr::PersistentRepr,
};

/// Actions to apply on the actor after journal response handling.
pub(crate) enum JournalResponseAction<A> {
  /// No actor callback required.
  None,
  /// Invoke the next pending handler.
  InvokeHandler(PendingHandlerInvocation<A>),
  /// Invoke multiple handlers in order.
  InvokeHandlers(Vec<PendingHandlerInvocation<A>>),
  /// Notify persist failure.
  PersistFailure { cause: JournalError, repr: PersistentRepr },
  /// Notify persist rejection.
  PersistRejected { cause: JournalError, repr: PersistentRepr },
  /// Deliver a replayed message.
  ReceiveRecover(PersistentRepr),
  /// Deliver replayed messages expanded by read adapters.
  ReceiveRecoverMany(Vec<PersistentRepr>),
  /// Notify recovery completion.
  RecoveryCompleted,
  /// Notify recovery failure.
  RecoveryFailure(PersistenceError),
}

impl<A> JournalResponseAction<A> {
  pub(crate) fn apply<TB: RuntimeToolbox + 'static>(self, actor: &mut A)
  where
    A: Eventsourced<TB>, {
    match self {
      | JournalResponseAction::None => {},
      | JournalResponseAction::InvokeHandler(invocation) => invocation.invoke(actor),
      | JournalResponseAction::InvokeHandlers(invocations) => {
        for invocation in invocations {
          invocation.invoke(actor);
        }
      },
      | JournalResponseAction::PersistFailure { cause, repr } => actor.on_persist_failure(&cause, &repr),
      | JournalResponseAction::PersistRejected { cause, repr } => actor.on_persist_rejected(&cause, &repr),
      | JournalResponseAction::ReceiveRecover(repr) => actor.receive_recover(&repr),
      | JournalResponseAction::ReceiveRecoverMany(reprs) => {
        for repr in reprs {
          actor.receive_recover(&repr);
        }
      },
      | JournalResponseAction::RecoveryCompleted => actor.on_recovery_completed(),
      | JournalResponseAction::RecoveryFailure(error) => actor.on_recovery_failure(&error),
    }
  }
}
