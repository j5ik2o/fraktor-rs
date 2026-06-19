//! Message dispatch facet for actor cells.

use alloc::boxed::Box;
use core::any::Any;

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, WeakShared};

use crate::{
  actor::{
    ActorCell,
    error::ActorError,
    messaging::{
      ActorIdentity, AnyMessage, Identify, Kill, PoisonPill,
      message_invoker::{MessageInvoker, MessageInvokerShared},
      system_message::{FailureMessageSnapshot, SystemMessage},
    },
  },
  dispatch::mailbox::{Mailbox, metrics_event::MailboxPressureEvent},
};

/// Installs the dispatcher invoker for the actor cell mailbox.
pub(super) fn install_invoker(cell: &ArcShared<ActorCell>, mailbox: &ArcShared<Mailbox>) {
  let invoker: MessageInvokerShared = MessageInvokerShared::new(Box::new(ActorCellInvoker { cell: cell.downgrade() }));
  mailbox.install_invoker(invoker);
}

/// Internal invoker that bridges dispatcher message delivery to actor cell.
///
/// Uses a weak reference to avoid circular reference between ActorCell and DispatcherCore.
pub(super) struct ActorCellInvoker {
  pub(super) cell: WeakShared<ActorCell>,
}

impl ActorCellInvoker {
  /// Upgrades the weak cell reference to a strong reference.
  ///
  /// Returns `None` if the actor cell has been dropped.
  fn cell(&self) -> Option<ArcShared<ActorCell>> {
    self.cell.upgrade()
  }
}

impl MessageInvoker for ActorCellInvoker {
  fn invoke(&mut self, message: AnyMessage) -> Result<(), ActorError> {
    let Some(cell) = self.cell() else {
      // ActorCell has been dropped, silently ignore the message
      return Ok(());
    };
    if cell.is_terminated() {
      return Ok(());
    }
    if message.payload().downcast_ref::<PoisonPill>().is_some() {
      return cell.handle_stop();
    }
    if message.payload().downcast_ref::<Kill>().is_some() {
      let snapshot = FailureMessageSnapshot::from_message(&message);
      return cell.handle_kill(Some(snapshot));
    }
    if let Some(system_message) = message.payload().downcast_ref::<SystemMessage>() {
      match system_message {
        | SystemMessage::PoisonPill => return cell.handle_stop(),
        | SystemMessage::Kill => {
          let snapshot = FailureMessageSnapshot::from_message(&message);
          return cell.handle_kill(Some(snapshot));
        },
        | _ => {},
      }
    }
    if let Some(identify) = message.payload().downcast_ref::<Identify>() {
      if let Some(mut sender) = message.sender().cloned() {
        let identity = ActorIdentity::found(identify.correlation_id().clone(), cell.actor_ref());
        // Best-effort reply: the requester may have stopped before the reply arrives.
        sender.try_tell(AnyMessage::new(identity)).map_err(|error| ActorError::from_send_error(&error))?;
      }
      // NOTE: No reply is sent if sender is None (no deadLetters in no_std).
      // Use with_sender() to receive ActorIdentity replies.
      return Ok(());
    }
    let mut ctx = cell.make_context();
    let failure_candidate = message.clone();
    let result = cell.actor.with_write(|actor| cell.pipeline.invoke_user(&mut **actor, &mut ctx, message));
    match &result {
      | Ok(()) => ActorCell::reschedule_receive_timeout_after_user_success(&mut ctx, &failure_candidate),
      | Err(error) => {
        let snapshot = FailureMessageSnapshot::from_message(&failure_candidate);
        cell.report_failure(error, Some(snapshot));
      },
    }
    result
  }

  fn system_invoke(&mut self, message: SystemMessage) -> Result<(), ActorError> {
    let Some(cell) = self.cell() else {
      // ActorCell has been dropped, silently ignore the message
      return Ok(());
    };
    if cell.is_terminated() {
      return Ok(());
    }
    match message {
      | SystemMessage::PoisonPill => cell.handle_stop(),
      | SystemMessage::Kill => {
        let payload: ArcShared<dyn Any + Send + Sync + 'static> = ArcShared::new(SystemMessage::Kill);
        let snapshot = FailureMessageSnapshot::new(payload, None);
        cell.handle_kill(Some(snapshot))
      },
      | SystemMessage::Stop => cell.handle_stop(),
      | SystemMessage::Create => cell.handle_create(),
      | SystemMessage::Recreate(cause) => cell.fault_recreate(&cause),
      | SystemMessage::Failure(ref payload) => {
        cell.handle_failure(payload);
        Ok(())
      },
      | SystemMessage::Suspend => {
        // Pekko `FaultHandling.scala:124-128` faultSuspend: the mailbox counter
        // has already been updated inside `Mailbox::process_all_system_messages`
        // (MB-H1); here we only perform the AC-H3 recursion into the children.
        cell.suspend_children();
        Ok(())
      },
      | SystemMessage::Resume => {
        // Pekko `FaultHandling.scala:136-153` faultResume: the mailbox counter
        // has already been decremented by the mailbox layer before forwarding
        // (MB-H1).
        //
        // AC-M3 (change pekko-fault-dispatcher-hardening): mirror Pekko's
        // `finally if (causedByFailure ne null) clearFailed()` at
        // `FaultHandling.scala:150`. Because `report_failure` now records
        // `FailedInfo::Child(self.pid)` via `set_failed` (Pekko L222),
        // receiving `Resume` must clear that state so `is_failed()` does not
        // stay stale across supervisor-approved resume directives.
        //
        // Known divergence from Pekko (Decision 5 in design.md):
        //   - Pekko's `clearFailed` (L83-86) preserves `FailedFatally`; fraktor-rs's `clear_failed()` is
        //     unconditional. Accepted because `SystemMessage::Resume` never reaches a cell that remained in
        //     `Fatal` state in production — the only `set_failed_fatally()` production call site is the
        //     `finish_recreate` post_restart-failure path, after which the supervisor typically chooses
        //     Restart/Stop, not Resume.
        //   - Pekko propagates `causedByFailure` through `resumeChildren` so only the originator clears
        //     `_failed`; fraktor-rs's `SystemMessage::Resume` carries no cause, so propagation into
        //     children that independently acquired `FailedInfo::Child(_)` state would over-clear. This race
        //     is narrow (no production readers of `perpetrator()` yet) and accepted for AC-M3 scope. A
        //     future `SystemMessage::Resume { cause: Option<...> }` refactor can restore strict Pekko
        //     parity.
        //
        // Ordering matches Pekko's `try resumeNonRecursive() finally
        // clearFailed(); resumeChildren(...)` — clear before propagation.
        cell.clear_failed();
        cell.resume_children();
        Ok(())
      },
      | SystemMessage::Watch(pid) => {
        cell.handle_watch(pid);
        Ok(())
      },
      | SystemMessage::Unwatch(pid) => {
        cell.handle_unwatch(pid);
        Ok(())
      },
      | SystemMessage::StopChild(pid) => {
        cell.stop_child(pid);
        Ok(())
      },
      | SystemMessage::DeathWatchNotification(pid) => cell.handle_death_watch_notification(pid),
      | SystemMessage::PipeTask(task_id) => {
        cell.handle_pipe_task_ready(task_id);
        Ok(())
      },
    }
  }

  fn invoke_mailbox_pressure(&mut self, event: &MailboxPressureEvent) -> Result<(), ActorError> {
    let Some(cell) = self.cell() else {
      // ActorCell has been dropped, silently ignore the notification
      return Ok(());
    };
    let mut ctx = cell.make_context();
    let result = cell.actor.with_write(|actor| actor.on_mailbox_pressure(&mut ctx, event));
    if let Err(ref error) = result {
      cell.report_failure(error, None);
    }
    result
  }
}
