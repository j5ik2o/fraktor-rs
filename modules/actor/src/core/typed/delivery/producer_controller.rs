//! Point-to-point reliable delivery producer controller.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::core::{
  error::ActorError,
  event::logging::LogLevel,
  typed::{
    Behaviors,
    actor::TypedActorRef,
    behavior::Behavior,
    delivery::{
      ConsumerControllerCommand, ProducerControllerCommand, ProducerControllerRequestNext, ProducerControllerSettings,
      SeqNr, SequencedMessage, producer_controller_command::ProducerControllerCommandKind,
    },
  },
};

/// Deferred side-effects that must be executed **after** the state lock is
/// released.  Collecting them avoids holding the lock while calling `tell()`,
/// which would deadlock when a message adapter routes a response back to the
/// same actor.
enum DeferredAction<A>
where
  A: Clone + Send + Sync + 'static, {
  RequestNext(TypedActorRef<ProducerControllerRequestNext<A>>, ProducerControllerRequestNext<A>),
  SendSequenced(TypedActorRef<ConsumerControllerCommand<A>>, ConsumerControllerCommand<A>),
}

struct ProducerControllerState<A>
where
  A: Clone + Send + Sync + 'static, {
  producer_id:         String,
  current_seq_nr:      SeqNr,
  confirmed_seq_nr:    SeqNr,
  requested_seq_nr:    SeqNr,
  requested:           bool,
  support_resend:      bool,
  unconfirmed:         Vec<SequencedMessage<A>>,
  producer:            Option<TypedActorRef<ProducerControllerRequestNext<A>>>,
  consumer_controller: Option<TypedActorRef<ConsumerControllerCommand<A>>>,
  send_adapter:        Option<TypedActorRef<A>>,
}

impl<A> ProducerControllerState<A>
where
  A: Clone + Send + Sync + 'static,
{
  const fn new(producer_id: String) -> Self {
    Self {
      producer_id,
      current_seq_nr: 1,
      confirmed_seq_nr: 0,
      requested_seq_nr: 0,
      requested: false,
      support_resend: true,
      unconfirmed: Vec::new(),
      producer: None,
      consumer_controller: None,
      send_adapter: None,
    }
  }

  const fn has_demand(&self) -> bool {
    self.current_seq_nr <= self.requested_seq_nr
  }

  fn on_confirmed(&mut self, confirmed_seq_nr: SeqNr) {
    if confirmed_seq_nr > self.confirmed_seq_nr {
      self.confirmed_seq_nr = confirmed_seq_nr;
      if self.support_resend {
        self.unconfirmed.retain(|msg| msg.seq_nr() > confirmed_seq_nr);
      }
    }
  }
}

/// Factory for creating a `ProducerController` behavior.
///
/// The `ProducerController` manages the producer side of point-to-point
/// reliable delivery. It works together with
/// [`ConsumerController`](super::ConsumerController) to provide
/// flow-controlled, sequence-numbered message delivery.
pub struct ProducerController;

impl ProducerController {
  /// Creates a `Start` command for the producer controller.
  #[must_use]
  pub const fn start<A>(producer: TypedActorRef<ProducerControllerRequestNext<A>>) -> ProducerControllerCommand<A>
  where
    A: Clone + Send + Sync + 'static, {
    ProducerControllerCommand::start(producer)
  }

  /// Creates a `RegisterConsumer` command.
  #[must_use]
  pub const fn register_consumer<A>(
    consumer_controller: TypedActorRef<ConsumerControllerCommand<A>>,
  ) -> ProducerControllerCommand<A>
  where
    A: Clone + Send + Sync + 'static, {
    ProducerControllerCommand::register_consumer(consumer_controller)
  }

  /// Creates the producer controller behavior with default settings.
  #[must_use]
  pub fn behavior<A>(producer_id: impl Into<String>) -> Behavior<ProducerControllerCommand<A>>
  where
    A: Clone + Send + Sync + 'static, {
    Self::behavior_with_settings(producer_id, ProducerControllerSettings::new())
  }

  /// Creates the producer controller behavior with custom settings.
  #[must_use]
  pub(crate) fn behavior_with_settings<A>(
    producer_id: impl Into<String>,
    _settings: ProducerControllerSettings,
  ) -> Behavior<ProducerControllerCommand<A>>
  where
    A: Clone + Send + Sync + 'static, {
    let producer_id = producer_id.into();

    Behaviors::setup(move |ctx| {
      let self_ref = ctx.self_ref();

      // Create a message adapter: A → ProducerControllerCommand::Msg
      let send_adapter = match ctx.message_adapter(|a: A| Ok(ProducerControllerCommand::msg(a))) {
        | Ok(adapter) => adapter,
        | Err(error) => {
          let message = alloc::format!("ProducerController failed to create send adapter: {:?}", error);
          ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()));
          return Behaviors::stopped();
        },
      };

      let state = ArcShared::new(RuntimeMutex::new(ProducerControllerState::<A>::new(producer_id.clone())));
      {
        let mut s = state.lock();
        s.send_adapter = Some(send_adapter);
      }

      Behaviors::receive_message(move |_ctx, command: &ProducerControllerCommand<A>| {
        // Collect deferred actions while holding the lock, then execute them
        // after releasing it to avoid re-entrant deadlock via message adapters.
        let deferred = {
          let mut state = state.lock();
          let mut deferred = Vec::new();
          match command.clone().into_kind() {
            | ProducerControllerCommandKind::Start { producer } => {
              state.producer = Some(producer);
              collect_request_next(&state, &mut deferred);
            },
            | ProducerControllerCommandKind::RegisterConsumer { consumer_controller } => {
              state.consumer_controller = Some(consumer_controller);
              collect_request_next(&state, &mut deferred);
            },
            | ProducerControllerCommandKind::Msg { message } => {
              collect_on_msg(&mut state, message, &self_ref, &mut deferred);
            },
            | ProducerControllerCommandKind::MsgWithConfirmation { message, .. } => {
              collect_on_msg(&mut state, message, &self_ref, &mut deferred);
            },
            | ProducerControllerCommandKind::Request {
              confirmed_seq_nr, request_up_to_seq_nr, support_resend, ..
            } => {
              state.support_resend = support_resend;
              state.on_confirmed(confirmed_seq_nr);
              state.requested_seq_nr = request_up_to_seq_nr;
              state.requested = true;
              collect_request_next(&state, &mut deferred);
            },
            | ProducerControllerCommandKind::Resend { from_seq_nr } => {
              collect_resend(&state, from_seq_nr, &mut deferred);
            },
            | ProducerControllerCommandKind::Ack { confirmed_seq_nr } => {
              state.on_confirmed(confirmed_seq_nr);
            },
            | ProducerControllerCommandKind::ResendFirstUnconfirmed => {
              if let Some(first) = state.unconfirmed.first().cloned()
                && let Some(cc) = state.consumer_controller.clone()
              {
                deferred
                  .push(DeferredAction::SendSequenced(cc, ConsumerControllerCommand::sequenced_msg(first.as_first())));
              }
            },
          }
          deferred
        }; // state lock released here

        execute_deferred(deferred)?;
        Ok(Behaviors::same())
      })
    })
  }
}

fn collect_request_next<A>(state: &ProducerControllerState<A>, deferred: &mut Vec<DeferredAction<A>>)
where
  A: Clone + Send + Sync + 'static, {
  if state.producer.is_none() || state.consumer_controller.is_none() {
    return;
  }
  let allow_first = !state.requested && state.current_seq_nr == 1;
  if (allow_first || state.has_demand())
    && let (Some(producer), Some(send_adapter)) = (&state.producer, &state.send_adapter)
  {
    let request_next = ProducerControllerRequestNext::new(
      state.producer_id.clone(),
      state.current_seq_nr,
      state.confirmed_seq_nr,
      send_adapter.clone(),
    );
    deferred.push(DeferredAction::RequestNext(producer.clone(), request_next));
  }
}

fn collect_on_msg<A>(
  state: &mut ProducerControllerState<A>,
  message: A,
  self_ref: &TypedActorRef<ProducerControllerCommand<A>>,
  deferred: &mut Vec<DeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  let seq_nr = state.current_seq_nr;
  let first = seq_nr == 1;

  let sequenced = SequencedMessage::new(state.producer_id.clone(), seq_nr, message, first, false, self_ref.clone());

  if state.support_resend {
    state.unconfirmed.push(sequenced.clone());
  }

  if let Some(cc) = state.consumer_controller.clone() {
    deferred.push(DeferredAction::SendSequenced(cc, ConsumerControllerCommand::sequenced_msg(sequenced)));
  }

  state.current_seq_nr += 1;
  // NOTE: do NOT call collect_request_next here.  With inline dispatch the
  // CC's Request arrives in the same processing chain, creating an infinite
  // feedback loop (PC→producer→PC→CC→PC).  Demand is refreshed only when
  // an explicit Request command arrives from the ConsumerController.
}

fn collect_resend<A>(state: &ProducerControllerState<A>, from_seq_nr: SeqNr, deferred: &mut Vec<DeferredAction<A>>)
where
  A: Clone + Send + Sync + 'static, {
  if let Some(cc) = state.consumer_controller.clone() {
    let to_resend: Vec<_> = state.unconfirmed.iter().filter(|msg| msg.seq_nr() >= from_seq_nr).cloned().collect();
    for (i, msg) in to_resend.into_iter().enumerate() {
      let msg = if i == 0 { msg.as_first() } else { msg };
      deferred.push(DeferredAction::SendSequenced(cc.clone(), ConsumerControllerCommand::sequenced_msg(msg)));
    }
  }
}

fn execute_deferred<A>(actions: Vec<DeferredAction<A>>) -> Result<(), ActorError>
where
  A: Clone + Send + Sync + 'static, {
  for action in actions {
    match action {
      | DeferredAction::RequestNext(mut target, msg) => {
        target.tell(msg).map_err(|e| ActorError::from_send_error(&e))?;
      },
      | DeferredAction::SendSequenced(mut target, msg) => {
        target.tell(msg).map_err(|e| ActorError::from_send_error(&e))?;
      },
    }
  }
  Ok(())
}
