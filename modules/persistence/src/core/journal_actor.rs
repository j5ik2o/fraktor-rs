//! Journal actor implementation.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::String, vec::Vec};
use core::{
  future::Future,
  marker::PhantomData,
  pin::Pin,
  task::{Context, Poll, Waker},
};

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
};
use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use crate::core::{
  journal::Journal, journal_actor_config::JournalActorConfig, journal_error::JournalError,
  journal_message::JournalMessage, journal_response::JournalResponse, persistent_repr::PersistentRepr,
};

struct JournalPoll;

enum JournalInFlight<TB: RuntimeToolbox + 'static> {
  Write {
    future:      Pin<Box<dyn Future<Output = Result<(), JournalError>> + Send>>,
    messages:    Vec<PersistentRepr>,
    sender:      ActorRefGeneric<TB>,
    instance_id: u32,
    retry_count: u32,
  },
  Replay {
    future:           Pin<Box<dyn Future<Output = Result<Vec<PersistentRepr>, JournalError>> + Send>>,
    sender:           ActorRefGeneric<TB>,
    persistence_id:   String,
    from_sequence_nr: u64,
    to_sequence_nr:   u64,
    max:              u64,
    retry_count:      u32,
  },
  Delete {
    future:         Pin<Box<dyn Future<Output = Result<(), JournalError>> + Send>>,
    sender:         ActorRefGeneric<TB>,
    persistence_id: String,
    to_sequence_nr: u64,
    retry_count:    u32,
  },
  Highest {
    future:         Pin<Box<dyn Future<Output = Result<u64, JournalError>> + Send>>,
    sender:         ActorRefGeneric<TB>,
    persistence_id: String,
    retry_count:    u32,
  },
}

/// Actor wrapper around a journal implementation.
pub struct JournalActor<J: Journal, TB: RuntimeToolbox + 'static> {
  journal:        J,
  in_flight:      Vec<JournalInFlight<TB>>,
  poll_scheduled: bool,
  config:         JournalActorConfig,
  _marker:        PhantomData<TB>,
}

impl<J: Journal, TB: RuntimeToolbox + 'static> JournalActor<J, TB>
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
    Self { journal, in_flight: Vec::new(), poll_scheduled: false, config, _marker: PhantomData }
  }

  fn schedule_poll(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) {
    if self.poll_scheduled || self.in_flight.is_empty() {
      return;
    }
    self.poll_scheduled = true;
    if ctx.self_ref().tell(AnyMessageGeneric::new(JournalPoll)).is_err() {
      // tell失敗時にフラグをリセットし、ポーリング停止を防ぐ
      self.poll_scheduled = false;
    }
  }

  fn poll_in_flight(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) {
    self.poll_scheduled = false;
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    let mut pending = Vec::new();
    let retry_max = self.config.retry_max();
    let in_flight = core::mem::take(&mut self.in_flight);
    for entry in in_flight {
      if let Some(entry) = poll_entry(&mut self.journal, entry, &mut cx, retry_max) {
        pending.push(entry);
      }
    }
    self.in_flight = pending;
    self.schedule_poll(ctx);
  }
}

impl<J: Journal, TB: RuntimeToolbox + 'static> Actor<TB> for JournalActor<J, TB>
where
  for<'a> J::WriteFuture<'a>: Send + 'static,
  for<'a> J::ReplayFuture<'a>: Send + 'static,
  for<'a> J::DeleteFuture<'a>: Send + 'static,
  for<'a> J::HighestSeqNrFuture<'a>: Send + 'static,
{
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<JournalPoll>().is_some() {
      self.poll_in_flight(ctx);
      return Ok(());
    }

    if let Some(msg) = message.downcast_ref::<JournalMessage<TB>>() {
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
      self.poll_in_flight(ctx);
    }
    Ok(())
  }
}

fn poll_entry<J: Journal, TB: RuntimeToolbox + 'static>(
  journal: &mut J,
  mut entry: JournalInFlight<TB>,
  cx: &mut Context<'_>,
  retry_max: u32,
) -> Option<JournalInFlight<TB>>
where
  for<'a> J::WriteFuture<'a>: Send + 'static,
  for<'a> J::ReplayFuture<'a>: Send + 'static,
  for<'a> J::DeleteFuture<'a>: Send + 'static,
  for<'a> J::HighestSeqNrFuture<'a>: Send + 'static, {
  match &mut entry {
    | JournalInFlight::Write { future, messages, sender, instance_id, retry_count } => {
      match Future::poll(future.as_mut(), cx) {
        | Poll::Ready(Ok(())) => {
          for repr in messages.iter().cloned() {
            let _ = sender
              .tell(AnyMessageGeneric::new(JournalResponse::WriteMessageSuccess { repr, instance_id: *instance_id }));
          }
          let _ =
            sender.tell(AnyMessageGeneric::new(JournalResponse::WriteMessagesSuccessful { instance_id: *instance_id }));
          None
        },
        | Poll::Ready(Err(error)) => {
          if *retry_count < retry_max {
            *retry_count = retry_count.saturating_add(1);
            *future = Box::pin(journal.write_messages(messages));
            Some(entry)
          } else {
            for repr in messages.iter().cloned() {
              let _ = sender.tell(AnyMessageGeneric::new(JournalResponse::WriteMessageFailure {
                repr,
                cause: error.clone(),
                instance_id: *instance_id,
              }));
            }
            let _ = sender.tell(AnyMessageGeneric::new(JournalResponse::WriteMessagesFailed {
              cause:       error,
              write_count: messages.len() as u64,
              instance_id: *instance_id,
            }));
            None
          }
        },
        | Poll::Pending => Some(entry),
      }
    },
    | JournalInFlight::Replay {
      future,
      sender,
      persistence_id,
      from_sequence_nr,
      to_sequence_nr,
      max,
      retry_count,
    } => match Future::poll(future.as_mut(), cx) {
      | Poll::Ready(Ok(messages)) => {
        let mut highest = 0;
        for repr in messages.iter().cloned() {
          highest = repr.sequence_nr();
          if repr.deleted() {
            continue;
          }
          let _ = sender.tell(AnyMessageGeneric::new(JournalResponse::ReplayedMessage { persistent_repr: repr }));
        }
        let _ = sender.tell(AnyMessageGeneric::new(JournalResponse::RecoverySuccess { highest_sequence_nr: highest }));
        None
      },
      | Poll::Ready(Err(error)) => {
        if *retry_count < retry_max {
          *retry_count = retry_count.saturating_add(1);
          *future = Box::pin(journal.replay_messages(persistence_id, *from_sequence_nr, *to_sequence_nr, *max));
          Some(entry)
        } else {
          let _ = sender.tell(AnyMessageGeneric::new(JournalResponse::ReplayMessagesFailure { cause: error }));
          None
        }
      },
      | Poll::Pending => Some(entry),
    },
    | JournalInFlight::Delete { future, sender, persistence_id, to_sequence_nr, retry_count } => {
      match Future::poll(future.as_mut(), cx) {
        | Poll::Ready(Ok(())) => {
          let _ = sender
            .tell(AnyMessageGeneric::new(JournalResponse::DeleteMessagesSuccess { to_sequence_nr: *to_sequence_nr }));
          None
        },
        | Poll::Ready(Err(error)) => {
          if *retry_count < retry_max {
            *retry_count = retry_count.saturating_add(1);
            *future = Box::pin(journal.delete_messages_to(persistence_id, *to_sequence_nr));
            Some(entry)
          } else {
            let _ = sender.tell(AnyMessageGeneric::new(JournalResponse::DeleteMessagesFailure {
              cause:          error,
              to_sequence_nr: *to_sequence_nr,
            }));
            None
          }
        },
        | Poll::Pending => Some(entry),
      }
    },
    | JournalInFlight::Highest { future, sender, persistence_id, retry_count } => {
      match Future::poll(future.as_mut(), cx) {
        | Poll::Ready(Ok(sequence_nr)) => {
          let _ = sender.tell(AnyMessageGeneric::new(JournalResponse::HighestSequenceNr {
            persistence_id: persistence_id.clone(),
            sequence_nr,
          }));
          None
        },
        | Poll::Ready(Err(error)) => {
          if *retry_count < retry_max {
            *retry_count = retry_count.saturating_add(1);
            *future = Box::pin(journal.highest_sequence_nr(persistence_id));
            Some(entry)
          } else {
            let _ = sender.tell(AnyMessageGeneric::new(JournalResponse::HighestSequenceNrFailure {
              persistence_id: persistence_id.clone(),
              cause:          error,
            }));
            None
          }
        },
        | Poll::Pending => Some(entry),
      }
    },
  }
}
