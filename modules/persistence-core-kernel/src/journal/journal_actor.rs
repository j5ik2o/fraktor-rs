//! Journal actor implementation.

#[cfg(test)]
#[path = "journal_actor_test.rs"]
mod tests;

use alloc::{boxed::Box, string::String, vec::Vec};
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll, Waker},
};

use fraktor_actor_core_kernel_rs::actor::{
  Actor, ActorContext,
  actor_ref::ActorRef,
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
};

use crate::{
  journal::{Journal, JournalActorConfig, JournalError, JournalMessage, JournalResponse},
  persistent::PersistentRepr,
};

struct JournalPoll;

type JournalWriteFuture = Pin<Box<dyn Future<Output = Result<(), JournalError>> + Send>>;
type JournalReplayFuture = Pin<Box<dyn Future<Output = Result<Vec<PersistentRepr>, JournalError>> + Send>>;
type JournalDeleteFuture = Pin<Box<dyn Future<Output = Result<(), JournalError>> + Send>>;
type JournalHighestFuture = Pin<Box<dyn Future<Output = Result<u64, JournalError>> + Send>>;

struct JournalPollContext<'a, J: Journal> {
  journal:   &'a mut J,
  retry_max: u32,
}

#[derive(Clone, Copy)]
struct JournalReplayRequest {
  from_sequence_nr: u64,
  to_sequence_nr:   u64,
  max:              u64,
}

enum JournalInFlight {
  Write {
    future:      JournalWriteFuture,
    messages:    Vec<PersistentRepr>,
    sender:      ActorRef,
    instance_id: u32,
    retry_count: u32,
  },
  Replay {
    future:           JournalReplayFuture,
    sender:           ActorRef,
    persistence_id:   String,
    from_sequence_nr: u64,
    to_sequence_nr:   u64,
    max:              u64,
    retry_count:      u32,
  },
  Delete {
    future:         JournalDeleteFuture,
    sender:         ActorRef,
    persistence_id: String,
    to_sequence_nr: u64,
    retry_count:    u32,
  },
  Highest {
    future:         JournalHighestFuture,
    sender:         ActorRef,
    persistence_id: String,
    retry_count:    u32,
  },
}

/// Actor wrapper around a journal implementation.
pub struct JournalActor<J: Journal> {
  journal:        J,
  in_flight:      Vec<JournalInFlight>,
  poll_scheduled: bool,
  config:         JournalActorConfig,
}

impl<J: Journal> JournalActor<J>
where
  for<'a> J::WriteFuture<'a>: Send + 'static,
  for<'a> J::ReplayFuture<'a>: Send + 'static,
  for<'a> J::DeleteFuture<'a>: Send + 'static,
  for<'a> J::HighestSeqNrFuture<'a>: Send + 'static,
{
  /// Creates a new journal actor.
  #[must_use]
  pub const fn new(journal: J) -> Self {
    Self::new_with_config(journal, JournalActorConfig::default_config())
  }

  /// Creates a new journal actor with configuration.
  #[must_use]
  pub const fn new_with_config(journal: J, config: JournalActorConfig) -> Self {
    Self { journal, in_flight: Vec::new(), poll_scheduled: false, config }
  }

  fn schedule_poll(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    if self.poll_scheduled || self.in_flight.is_empty() {
      return Ok(());
    }
    self.poll_scheduled = true;
    ctx.self_ref().try_tell(AnyMessage::new(JournalPoll)).map_err(|error| ActorError::from_send_error(&error))
  }

  fn poll_in_flight(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.poll_scheduled = false;
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    let mut pending = Vec::new();
    let retry_max = self.config.retry_max();
    let in_flight = core::mem::take(&mut self.in_flight);
    for entry in in_flight {
      if let Some(entry) = poll_entry(&mut self.journal, entry, &mut cx, retry_max)? {
        pending.push(entry);
      }
    }
    self.in_flight = pending;
    self.schedule_poll(ctx)
  }
}

impl<J: Journal> Actor for JournalActor<J>
where
  for<'a> J::WriteFuture<'a>: Send + 'static,
  for<'a> J::ReplayFuture<'a>: Send + 'static,
  for<'a> J::DeleteFuture<'a>: Send + 'static,
  for<'a> J::HighestSeqNrFuture<'a>: Send + 'static,
{
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<JournalPoll>().is_some() {
      self.poll_in_flight(ctx)?;
      return Ok(());
    }

    if let Some(msg) = message.downcast_ref::<JournalMessage>() {
      match msg {
        | JournalMessage::WriteMessages { messages, sender, instance_id, .. } => {
          let future = Box::pin(self.journal.write_messages(messages));
          self.in_flight.push(JournalInFlight::Write {
            future,
            messages: messages.clone(),
            sender: sender.clone(),
            instance_id: *instance_id,
            retry_count: 0,
          });
        },
        | JournalMessage::ReplayMessages { persistence_id, from_sequence_nr, to_sequence_nr, max, sender } => {
          let future = Box::pin(self.journal.replay_messages(persistence_id, *from_sequence_nr, *to_sequence_nr, *max));
          self.in_flight.push(JournalInFlight::Replay {
            future,
            sender: sender.clone(),
            persistence_id: persistence_id.clone(),
            from_sequence_nr: *from_sequence_nr,
            to_sequence_nr: *to_sequence_nr,
            max: *max,
            retry_count: 0,
          });
        },
        | JournalMessage::DeleteMessagesTo { persistence_id, to_sequence_nr, sender } => {
          let future = Box::pin(self.journal.delete_messages_to(persistence_id, *to_sequence_nr));
          self.in_flight.push(JournalInFlight::Delete {
            future,
            sender: sender.clone(),
            persistence_id: persistence_id.clone(),
            to_sequence_nr: *to_sequence_nr,
            retry_count: 0,
          });
        },
        | JournalMessage::GetHighestSequenceNr { persistence_id, sender, .. } => {
          let future = Box::pin(self.journal.highest_sequence_nr(persistence_id));
          self.in_flight.push(JournalInFlight::Highest {
            future,
            sender: sender.clone(),
            persistence_id: persistence_id.clone(),
            retry_count: 0,
          });
        },
      }
      self.poll_in_flight(ctx)?;
    }
    Ok(())
  }
}

/// Returns `Some(entry)` when the in-flight operation is still pending.
fn poll_entry<J: Journal>(
  journal: &mut J,
  mut entry: JournalInFlight,
  cx: &mut Context<'_>,
  retry_max: u32,
) -> Result<Option<JournalInFlight>, ActorError>
where
  for<'a> J::WriteFuture<'a>: Send + 'static,
  for<'a> J::ReplayFuture<'a>: Send + 'static,
  for<'a> J::DeleteFuture<'a>: Send + 'static,
  for<'a> J::HighestSeqNrFuture<'a>: Send + 'static, {
  let mut poll_context = JournalPollContext { journal, retry_max };
  let keep_pending = match &mut entry {
    | JournalInFlight::Write { future, messages, sender, instance_id, retry_count } => {
      poll_write_entry(&mut poll_context, cx, future, messages, sender, *instance_id, retry_count)?
    },
    | JournalInFlight::Replay {
      future,
      sender,
      persistence_id,
      from_sequence_nr,
      to_sequence_nr,
      max,
      retry_count,
    } => {
      let request = JournalReplayRequest {
        from_sequence_nr: *from_sequence_nr,
        to_sequence_nr:   *to_sequence_nr,
        max:              *max,
      };
      poll_replay_entry(&mut poll_context, cx, future, sender, persistence_id, request, retry_count)?
    },
    | JournalInFlight::Delete { future, sender, persistence_id, to_sequence_nr, retry_count } => {
      poll_delete_entry(&mut poll_context, cx, future, sender, persistence_id, *to_sequence_nr, retry_count)?
    },
    | JournalInFlight::Highest { future, sender, persistence_id, retry_count } => {
      poll_highest_entry(&mut poll_context, cx, future, sender, persistence_id, retry_count)?
    },
  };

  if keep_pending { Ok(Some(entry)) } else { Ok(None) }
}

fn poll_write_entry<J: Journal>(
  poll_context: &mut JournalPollContext<'_, J>,
  cx: &mut Context<'_>,
  future: &mut JournalWriteFuture,
  messages: &[PersistentRepr],
  sender: &mut ActorRef,
  instance_id: u32,
  retry_count: &mut u32,
) -> Result<bool, ActorError>
where
  for<'a> J::WriteFuture<'a>: Send + 'static, {
  match Future::poll(future.as_mut(), cx) {
    | Poll::Ready(Ok(())) => {
      send_write_success(sender, messages, instance_id)?;
      Ok(false)
    },
    | Poll::Ready(Err(error)) => {
      retry_or_fail_write(poll_context, future, messages, sender, instance_id, retry_count, error)
    },
    | Poll::Pending => Ok(true),
  }
}

fn send_write_success(sender: &mut ActorRef, messages: &[PersistentRepr], instance_id: u32) -> Result<(), ActorError> {
  for repr in messages.iter().cloned() {
    sender
      .try_tell(AnyMessage::new(JournalResponse::WriteMessageSuccess { repr, instance_id }))
      .map_err(|error| ActorError::from_send_error(&error))?;
  }
  sender
    .try_tell(AnyMessage::new(JournalResponse::WriteMessagesSuccessful { instance_id }))
    .map_err(|error| ActorError::from_send_error(&error))
}

fn retry_or_fail_write<J: Journal>(
  poll_context: &mut JournalPollContext<'_, J>,
  future: &mut JournalWriteFuture,
  messages: &[PersistentRepr],
  sender: &mut ActorRef,
  instance_id: u32,
  retry_count: &mut u32,
  error: JournalError,
) -> Result<bool, ActorError>
where
  for<'a> J::WriteFuture<'a>: Send + 'static, {
  if *retry_count < poll_context.retry_max {
    *retry_count = retry_count.saturating_add(1);
    *future = Box::pin(poll_context.journal.write_messages(messages));
    return Ok(true);
  }
  send_write_failure(sender, messages, instance_id, error)?;
  Ok(false)
}

fn send_write_failure(
  sender: &mut ActorRef,
  messages: &[PersistentRepr],
  instance_id: u32,
  error: JournalError,
) -> Result<(), ActorError> {
  for repr in messages.iter().cloned() {
    sender
      .try_tell(AnyMessage::new(JournalResponse::WriteMessageFailure { repr, cause: error.clone(), instance_id }))
      .map_err(|send_error| ActorError::from_send_error(&send_error))?;
  }
  sender
    .try_tell(AnyMessage::new(JournalResponse::WriteMessagesFailed {
      cause: error,
      write_count: messages.len() as u64,
      instance_id,
    }))
    .map_err(|send_error| ActorError::from_send_error(&send_error))
}

fn poll_replay_entry<J: Journal>(
  poll_context: &mut JournalPollContext<'_, J>,
  cx: &mut Context<'_>,
  future: &mut JournalReplayFuture,
  sender: &mut ActorRef,
  persistence_id: &str,
  request: JournalReplayRequest,
  retry_count: &mut u32,
) -> Result<bool, ActorError>
where
  for<'a> J::ReplayFuture<'a>: Send + 'static, {
  match Future::poll(future.as_mut(), cx) {
    | Poll::Ready(Ok(messages)) => {
      send_replay_success(sender, &messages)?;
      Ok(false)
    },
    | Poll::Ready(Err(error)) => {
      retry_or_fail_replay(poll_context, future, sender, persistence_id, request, retry_count, error)
    },
    | Poll::Pending => Ok(true),
  }
}

fn send_replay_success(sender: &mut ActorRef, messages: &[PersistentRepr]) -> Result<(), ActorError> {
  let mut highest = 0;
  for repr in messages.iter().cloned() {
    highest = repr.sequence_nr();
    if repr.deleted() {
      continue;
    }
    sender
      .try_tell(AnyMessage::new(JournalResponse::ReplayedMessage { persistent_repr: repr }))
      .map_err(|error| ActorError::from_send_error(&error))?;
  }
  sender
    .try_tell(AnyMessage::new(JournalResponse::RecoverySuccess { highest_sequence_nr: highest }))
    .map_err(|error| ActorError::from_send_error(&error))
}

fn retry_or_fail_replay<J: Journal>(
  poll_context: &mut JournalPollContext<'_, J>,
  future: &mut JournalReplayFuture,
  sender: &mut ActorRef,
  persistence_id: &str,
  request: JournalReplayRequest,
  retry_count: &mut u32,
  error: JournalError,
) -> Result<bool, ActorError>
where
  for<'a> J::ReplayFuture<'a>: Send + 'static, {
  if *retry_count < poll_context.retry_max {
    *retry_count = retry_count.saturating_add(1);
    *future = Box::pin(poll_context.journal.replay_messages(
      persistence_id,
      request.from_sequence_nr,
      request.to_sequence_nr,
      request.max,
    ));
    return Ok(true);
  }
  sender
    .try_tell(AnyMessage::new(JournalResponse::ReplayMessagesFailure { cause: error }))
    .map_err(|send_error| ActorError::from_send_error(&send_error))?;
  Ok(false)
}

fn poll_delete_entry<J: Journal>(
  poll_context: &mut JournalPollContext<'_, J>,
  cx: &mut Context<'_>,
  future: &mut JournalDeleteFuture,
  sender: &mut ActorRef,
  persistence_id: &str,
  to_sequence_nr: u64,
  retry_count: &mut u32,
) -> Result<bool, ActorError>
where
  for<'a> J::DeleteFuture<'a>: Send + 'static, {
  match Future::poll(future.as_mut(), cx) {
    | Poll::Ready(Ok(())) => {
      sender
        .try_tell(AnyMessage::new(JournalResponse::DeleteMessagesSuccess { to_sequence_nr }))
        .map_err(|error| ActorError::from_send_error(&error))?;
      Ok(false)
    },
    | Poll::Ready(Err(error)) => {
      retry_or_fail_delete(poll_context, future, sender, persistence_id, to_sequence_nr, retry_count, error)
    },
    | Poll::Pending => Ok(true),
  }
}

fn retry_or_fail_delete<J: Journal>(
  poll_context: &mut JournalPollContext<'_, J>,
  future: &mut JournalDeleteFuture,
  sender: &mut ActorRef,
  persistence_id: &str,
  to_sequence_nr: u64,
  retry_count: &mut u32,
  error: JournalError,
) -> Result<bool, ActorError>
where
  for<'a> J::DeleteFuture<'a>: Send + 'static, {
  if *retry_count < poll_context.retry_max {
    *retry_count = retry_count.saturating_add(1);
    *future = Box::pin(poll_context.journal.delete_messages_to(persistence_id, to_sequence_nr));
    return Ok(true);
  }
  sender
    .try_tell(AnyMessage::new(JournalResponse::DeleteMessagesFailure { cause: error, to_sequence_nr }))
    .map_err(|send_error| ActorError::from_send_error(&send_error))?;
  Ok(false)
}

fn poll_highest_entry<J: Journal>(
  poll_context: &mut JournalPollContext<'_, J>,
  cx: &mut Context<'_>,
  future: &mut JournalHighestFuture,
  sender: &mut ActorRef,
  persistence_id: &str,
  retry_count: &mut u32,
) -> Result<bool, ActorError>
where
  for<'a> J::HighestSeqNrFuture<'a>: Send + 'static, {
  match Future::poll(future.as_mut(), cx) {
    | Poll::Ready(Ok(sequence_nr)) => {
      sender
        .try_tell(AnyMessage::new(JournalResponse::HighestSequenceNr {
          persistence_id: persistence_id.into(),
          sequence_nr,
        }))
        .map_err(|error| ActorError::from_send_error(&error))?;
      Ok(false)
    },
    | Poll::Ready(Err(error)) => {
      retry_or_fail_highest(poll_context, future, sender, persistence_id, retry_count, error)
    },
    | Poll::Pending => Ok(true),
  }
}

fn retry_or_fail_highest<J: Journal>(
  poll_context: &mut JournalPollContext<'_, J>,
  future: &mut JournalHighestFuture,
  sender: &mut ActorRef,
  persistence_id: &str,
  retry_count: &mut u32,
  error: JournalError,
) -> Result<bool, ActorError>
where
  for<'a> J::HighestSeqNrFuture<'a>: Send + 'static, {
  if *retry_count < poll_context.retry_max {
    *retry_count = retry_count.saturating_add(1);
    *future = Box::pin(poll_context.journal.highest_sequence_nr(persistence_id));
    return Ok(true);
  }
  sender
    .try_tell(AnyMessage::new(JournalResponse::HighestSequenceNrFailure {
      persistence_id: persistence_id.into(),
      cause:          error,
    }))
    .map_err(|send_error| ActorError::from_send_error(&send_error))?;
  Ok(false)
}
