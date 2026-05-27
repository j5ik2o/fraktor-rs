//! Actor proxy for forwarding persistence plugin protocol messages.

use alloc::string::String;

use fraktor_actor_core_kernel_rs::actor::{
  Actor, ActorContext,
  actor_ref::ActorRef,
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
};

use crate::{
  journal::{JournalError, JournalMessage, JournalResponse, PersistencePluginProxyCommand},
  persistent::{AtomicWrite, PersistentRepr},
  snapshot::{SnapshotError, SnapshotMessage, SnapshotResponse},
};

const JOURNAL_TARGET_NOT_SET: &str = "journal plugin target is not set";
const JOURNAL_TARGET_FORWARD_FAILED: &str = "journal plugin target forwarding failed";
const SNAPSHOT_TARGET_NOT_SET: &str = "snapshot plugin target is not set";
const SNAPSHOT_TARGET_FORWARD_FAILED: &str = "snapshot plugin target forwarding failed";

/// Actor proxy that forwards persistence plugin protocol messages to configured targets.
pub struct PersistencePluginProxyActor {
  journal_target:  Option<ActorRef>,
  snapshot_target: Option<ActorRef>,
}

impl PersistencePluginProxyActor {
  /// Creates a proxy actor without configured plugin targets.
  #[must_use]
  pub const fn new() -> Self {
    Self { journal_target: None, snapshot_target: None }
  }

  fn handle_command(&mut self, command: &PersistencePluginProxyCommand) {
    match command {
      | PersistencePluginProxyCommand::SetJournalTarget { target } => {
        self.journal_target = Some(target.clone());
      },
      | PersistencePluginProxyCommand::SetSnapshotTarget { target } => {
        self.snapshot_target = Some(target.clone());
      },
    }
  }

  fn forward_journal_message(
    &mut self,
    ctx: &mut ActorContext<'_>,
    message: &JournalMessage,
  ) -> Result<(), ActorError> {
    match &mut self.journal_target {
      | Some(target) => match ctx.try_forward(target, AnyMessage::new(message.clone())) {
        | Ok(()) => Ok(()),
        | Err(_) => {
          reply_journal_failure(message, JOURNAL_TARGET_FORWARD_FAILED);
          Ok(())
        },
      },
      | None => {
        reply_journal_failure(message, JOURNAL_TARGET_NOT_SET);
        Ok(())
      },
    }
  }

  fn forward_snapshot_message(
    &mut self,
    ctx: &mut ActorContext<'_>,
    message: &SnapshotMessage,
  ) -> Result<(), ActorError> {
    match &mut self.snapshot_target {
      | Some(target) => match ctx.try_forward(target, AnyMessage::new(message.clone())) {
        | Ok(()) => Ok(()),
        | Err(_) => {
          reply_snapshot_failure(message, SNAPSHOT_TARGET_FORWARD_FAILED);
          Ok(())
        },
      },
      | None => {
        reply_snapshot_failure(message, SNAPSHOT_TARGET_NOT_SET);
        Ok(())
      },
    }
  }
}

impl Default for PersistencePluginProxyActor {
  fn default() -> Self {
    Self::new()
  }
}

impl Actor for PersistencePluginProxyActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<PersistencePluginProxyCommand>() {
      self.handle_command(command);
      return Ok(());
    }

    if let Some(journal_message) = message.downcast_ref::<JournalMessage>() {
      return self.forward_journal_message(ctx, journal_message);
    }

    if let Some(snapshot_message) = message.downcast_ref::<SnapshotMessage>() {
      return self.forward_snapshot_message(ctx, snapshot_message);
    }

    Ok(())
  }
}

fn reply_journal_failure(message: &JournalMessage, cause_message: &str) {
  match message {
    | JournalMessage::WriteMessages { messages, sender, instance_id, .. } => {
      let mut sender = sender.clone();
      let cause = JournalError::WriteFailed(String::from(cause_message));
      for repr in atomic_write_payloads(messages) {
        tell_journal_response(&mut sender, JournalResponse::WriteMessageFailure {
          repr,
          cause: cause.clone(),
          instance_id: *instance_id,
        });
      }
      tell_journal_response(&mut sender, JournalResponse::WriteMessagesFailed {
        cause,
        write_count: atomic_write_payload_count(messages),
        instance_id: *instance_id,
      });
    },
    | JournalMessage::ReplayMessages { sender, .. } => {
      let mut sender = sender.clone();
      tell_journal_response(&mut sender, JournalResponse::ReplayMessagesFailure {
        cause: JournalError::ReadFailed(String::from(cause_message)),
      });
    },
    | JournalMessage::DeleteMessagesTo { to_sequence_nr, sender, .. } => {
      let mut sender = sender.clone();
      tell_journal_response(&mut sender, JournalResponse::DeleteMessagesFailure {
        cause:          JournalError::DeleteFailed(String::from(cause_message)),
        to_sequence_nr: *to_sequence_nr,
      });
    },
    | JournalMessage::GetHighestSequenceNr { persistence_id, sender, .. } => {
      let mut sender = sender.clone();
      tell_journal_response(&mut sender, JournalResponse::HighestSequenceNrFailure {
        persistence_id: persistence_id.clone(),
        cause:          JournalError::ReadFailed(String::from(cause_message)),
      });
    },
  }
}

fn reply_snapshot_failure(message: &SnapshotMessage, cause_message: &str) {
  match message {
    | SnapshotMessage::SaveSnapshot { metadata, sender, .. } => {
      let mut sender = sender.clone();
      tell_snapshot_response(&mut sender, SnapshotResponse::SaveSnapshotFailure {
        metadata: metadata.clone(),
        error:    SnapshotError::SaveFailed(String::from(cause_message)),
      });
    },
    | SnapshotMessage::LoadSnapshot { sender, .. } => {
      let mut sender = sender.clone();
      tell_snapshot_response(&mut sender, SnapshotResponse::LoadSnapshotFailed {
        error: SnapshotError::LoadFailed(String::from(cause_message)),
      });
    },
    | SnapshotMessage::DeleteSnapshot { metadata, sender } => {
      let mut sender = sender.clone();
      tell_snapshot_response(&mut sender, SnapshotResponse::DeleteSnapshotFailure {
        metadata: metadata.clone(),
        error:    SnapshotError::DeleteFailed(String::from(cause_message)),
      });
    },
    | SnapshotMessage::DeleteSnapshots { criteria, sender, .. } => {
      let mut sender = sender.clone();
      tell_snapshot_response(&mut sender, SnapshotResponse::DeleteSnapshotsFailure {
        criteria: criteria.clone(),
        error:    SnapshotError::DeleteFailed(String::from(cause_message)),
      });
    },
  }
}

fn atomic_write_payloads(messages: &[AtomicWrite]) -> impl Iterator<Item = PersistentRepr> + '_ {
  messages.iter().flat_map(AtomicWrite::payload).cloned()
}

fn atomic_write_payload_count(messages: &[AtomicWrite]) -> u64 {
  messages.iter().map(AtomicWrite::size).sum::<usize>() as u64
}

fn tell_journal_response(sender: &mut ActorRef, response: JournalResponse) {
  let _ = sender.try_tell(AnyMessage::new(response));
}

fn tell_snapshot_response(sender: &mut ActorRef, response: SnapshotResponse) {
  let _ = sender.try_tell(AnyMessage::new(response));
}
