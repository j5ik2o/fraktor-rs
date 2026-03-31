//! Point-to-point reliable delivery consumer controller.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::core::{
  kernel::event::logging::LogLevel,
  typed::{
    TypedActorRef,
    behavior::Behavior,
    delivery::{
      ConsumerControllerCommand, ConsumerControllerConfirmed, ConsumerControllerDelivery, ConsumerControllerSettings,
      ProducerControllerCommand, SeqNr, SequencedMessage, consumer_controller_command::ConsumerControllerCommandKind,
    },
    dsl::Behaviors,
  },
};

/// Deferred side-effects executed after releasing the state lock.
enum DeferredAction<A>
where
  A: Clone + Send + Sync + 'static, {
  SendToProducer(TypedActorRef<ProducerControllerCommand<A>>, ProducerControllerCommand<A>),
  Deliver(TypedActorRef<ConsumerControllerDelivery<A>>, ConsumerControllerDelivery<A>),
}

struct ConsumerControllerState<A>
where
  A: Clone + Send + Sync + 'static, {
  settings:            ConsumerControllerSettings,
  received_seq_nr:     SeqNr,
  delivered_seq_nr:    SeqNr,
  confirmed_seq_nr:    SeqNr,
  requested_seq_nr:    SeqNr,
  deliver_to:          Option<TypedActorRef<ConsumerControllerDelivery<A>>>,
  producer_controller: Option<TypedActorRef<ProducerControllerCommand<A>>>,
  waiting_for_confirm: bool,
  stashed:             Vec<SequencedMessage<A>>,
  stopping:            bool,
}

impl<A> ConsumerControllerState<A>
where
  A: Clone + Send + Sync + 'static,
{
  const fn new(settings: ConsumerControllerSettings) -> Self {
    Self {
      settings,
      received_seq_nr: 0,
      delivered_seq_nr: 0,
      confirmed_seq_nr: 0,
      requested_seq_nr: 0,
      deliver_to: None,
      producer_controller: None,
      waiting_for_confirm: false,
      stashed: Vec::new(),
      stopping: false,
    }
  }

  const fn is_next_expected(&self, seq_nr: SeqNr) -> bool {
    seq_nr == self.received_seq_nr + 1
  }

  fn should_request_more(&self) -> bool {
    let remaining = self.requested_seq_nr.saturating_sub(self.received_seq_nr);
    let window = u64::from(self.settings.flow_control_window());
    remaining <= window / 2
  }
}

/// Factory for creating a `ConsumerController` behavior.
///
/// The `ConsumerController` manages the consumer side of point-to-point
/// reliable delivery. It works together with
/// [`ProducerController`](super::ProducerController) to provide
/// flow-controlled, sequence-numbered message delivery.
pub struct ConsumerController;

impl ConsumerController {
  /// Creates a `Start` command for the consumer controller.
  #[must_use]
  pub const fn start<A>(deliver_to: TypedActorRef<ConsumerControllerDelivery<A>>) -> ConsumerControllerCommand<A>
  where
    A: Clone + Send + Sync + 'static, {
    ConsumerControllerCommand::start(deliver_to)
  }

  /// Creates a `RegisterToProducerController` command.
  #[must_use]
  pub const fn register_to_producer_controller<A>(
    producer_controller: TypedActorRef<ProducerControllerCommand<A>>,
  ) -> ConsumerControllerCommand<A>
  where
    A: Clone + Send + Sync + 'static, {
    ConsumerControllerCommand::register_to_producer_controller(producer_controller)
  }

  /// Creates a `Confirmed` command.
  #[must_use]
  pub const fn confirmed<A>() -> ConsumerControllerCommand<A>
  where
    A: Clone + Send + Sync + 'static, {
    ConsumerControllerCommand::confirmed()
  }

  /// Creates a `DeliverThenStop` command.
  #[must_use]
  pub const fn deliver_then_stop<A>() -> ConsumerControllerCommand<A>
  where
    A: Clone + Send + Sync + 'static, {
    ConsumerControllerCommand::deliver_then_stop()
  }

  /// Creates the consumer controller behavior with default settings.
  #[must_use]
  pub fn behavior<A>() -> Behavior<ConsumerControllerCommand<A>>
  where
    A: Clone + Send + Sync + 'static, {
    Self::behavior_with_settings(ConsumerControllerSettings::new())
  }

  /// Creates the consumer controller behavior with custom settings.
  #[must_use]
  pub fn behavior_with_settings<A>(settings: ConsumerControllerSettings) -> Behavior<ConsumerControllerCommand<A>>
  where
    A: Clone + Send + Sync + 'static, {
    let state = ArcShared::new(RuntimeMutex::new(ConsumerControllerState::<A>::new(settings)));

    Behaviors::setup(move |ctx| {
      let self_ref = ctx.self_ref();
      let confirm_adapter =
        match ctx.message_adapter(|_: ConsumerControllerConfirmed| Ok(ConsumerControllerCommand::confirmed())) {
          | Ok(adapter) => adapter,
          | Err(error) => {
            let message = alloc::format!("ConsumerController failed to create confirm adapter: {:?}", error);
            ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()));
            return Behaviors::stopped();
          },
        };
      let state_for_msg = state.clone();

      Behaviors::receive_message(move |_ctx, command: &ConsumerControllerCommand<A>| {
        let (deferred, should_stop) = {
          let mut state = state_for_msg.lock();
          let mut deferred = Vec::new();
          let mut should_stop = false;

          match command.kind() {
            | ConsumerControllerCommandKind::Start { deliver_to } => {
              state.deliver_to = Some(deliver_to.clone());
              collect_try_deliver_stashed(&mut state, &confirm_adapter, &mut deferred);
            },
            | ConsumerControllerCommandKind::RegisterToProducerController { producer_controller } => {
              let register_cmd = ProducerControllerCommand::register_consumer(self_ref.clone());
              deferred.push(DeferredAction::SendToProducer(producer_controller.clone(), register_cmd));
              state.producer_controller = Some(producer_controller.clone());
            },
            | ConsumerControllerCommandKind::SequencedMsg(seq_msg) => {
              collect_on_sequenced_message(&mut state, seq_msg.clone(), &self_ref, &confirm_adapter, &mut deferred);
            },
            | ConsumerControllerCommandKind::Confirmed => {
              collect_on_confirmed(&mut state, &self_ref, &confirm_adapter, &mut deferred);
            },
            | ConsumerControllerCommandKind::DeliverThenStop => {
              state.stopping = true;
              if !state.waiting_for_confirm && state.stashed.is_empty() {
                should_stop = true;
              }
            },
            | ConsumerControllerCommandKind::Retry => {
              collect_send_request(&mut state, &self_ref, true, &mut deferred);
            },
            | ConsumerControllerCommandKind::ConsumerTerminated => {
              should_stop = true;
            },
          }
          if !should_stop && state.stopping && !state.waiting_for_confirm && state.stashed.is_empty() {
            should_stop = true;
          }
          (deferred, should_stop)
        }; // state lock released here

        execute_deferred(deferred);
        if should_stop {
          return Ok(Behaviors::stopped());
        }
        Ok(Behaviors::same())
      })
    })
  }
}

fn collect_on_sequenced_message<A>(
  state: &mut ConsumerControllerState<A>,
  seq_msg: SequencedMessage<A>,
  self_ref: &TypedActorRef<ConsumerControllerCommand<A>>,
  confirm_adapter: &TypedActorRef<ConsumerControllerConfirmed>,
  deferred: &mut Vec<DeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  if seq_msg.first() {
    state.producer_controller = Some(TypedActorRef::<ProducerControllerCommand<A>>::from_untyped(
      seq_msg.producer_controller().as_untyped().clone(),
    ));
    state.received_seq_nr = seq_msg.seq_nr().saturating_sub(1);
    state.delivered_seq_nr = seq_msg.seq_nr().saturating_sub(1);
    state.confirmed_seq_nr = seq_msg.seq_nr().saturating_sub(1);
    state.stashed.clear();
    state.waiting_for_confirm = false;
    state.requested_seq_nr = 0;
  }

  let seq_nr = seq_msg.seq_nr();

  if seq_nr <= state.confirmed_seq_nr {
    return;
  }

  if state.is_next_expected(seq_nr) {
    state.received_seq_nr = seq_nr;
    if state.deliver_to.is_some() && !state.waiting_for_confirm {
      collect_deliver_to_consumer(state, &seq_msg, confirm_adapter, deferred);
    } else {
      state.stashed.push(seq_msg);
    }
  } else if seq_nr <= state.received_seq_nr {
    // 受信済みだが未確認の重複メッセージ — 無視する
    return;
  } else if seq_nr > state.received_seq_nr + 1 {
    // ギャップ検出: 到着メッセージをスタッシュしてリセンドを要求する。
    // Pekko と同様に、既に受信したメッセージを破棄せず保持し、
    // ギャップが埋まった後に順序通り配信する。
    state.stashed.push(seq_msg);
    if !state.settings.only_flow_control()
      && let Some(pc) = state.producer_controller.clone()
    {
      deferred.push(DeferredAction::SendToProducer(pc, ProducerControllerCommand::resend(state.received_seq_nr + 1)));
    }
    return;
  }

  if state.should_request_more() || state.requested_seq_nr == 0 {
    collect_send_request(state, self_ref, false, deferred);
  }
}

fn collect_deliver_to_consumer<A>(
  state: &mut ConsumerControllerState<A>,
  seq_msg: &SequencedMessage<A>,
  confirm_adapter: &TypedActorRef<ConsumerControllerConfirmed>,
  deferred: &mut Vec<DeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  if let Some(deliver_to) = state.deliver_to.clone() {
    let delivery = ConsumerControllerDelivery::new(
      seq_msg.message().clone(),
      confirm_adapter.clone(),
      seq_msg.producer_id().into(),
      seq_msg.seq_nr(),
    );
    deferred.push(DeferredAction::Deliver(deliver_to, delivery));
    state.delivered_seq_nr = seq_msg.seq_nr();
    state.waiting_for_confirm = true;
  }
}

fn collect_on_confirmed<A>(
  state: &mut ConsumerControllerState<A>,
  self_ref: &TypedActorRef<ConsumerControllerCommand<A>>,
  confirm_adapter: &TypedActorRef<ConsumerControllerConfirmed>,
  deferred: &mut Vec<DeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  state.confirmed_seq_nr = state.delivered_seq_nr;
  state.waiting_for_confirm = false;

  if let Some(pc) = state.producer_controller.clone() {
    deferred.push(DeferredAction::SendToProducer(pc, ProducerControllerCommand::ack(state.confirmed_seq_nr)));
  }

  collect_try_deliver_stashed(state, confirm_adapter, deferred);

  if state.should_request_more() {
    collect_send_request(state, self_ref, false, deferred);
  }
}

fn collect_try_deliver_stashed<A>(
  state: &mut ConsumerControllerState<A>,
  confirm_adapter: &TypedActorRef<ConsumerControllerConfirmed>,
  deferred: &mut Vec<DeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  if state.deliver_to.is_none() || state.waiting_for_confirm {
    return;
  }
  let expected = state.delivered_seq_nr + 1;
  if let Some(pos) = state.stashed.iter().position(|m| m.seq_nr() == expected) {
    let next = state.stashed.remove(pos);
    collect_deliver_to_consumer(state, &next, confirm_adapter, deferred);
  }
}

fn collect_send_request<A>(
  state: &mut ConsumerControllerState<A>,
  _self_ref: &TypedActorRef<ConsumerControllerCommand<A>>,
  via_timeout: bool,
  deferred: &mut Vec<DeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  let window = u64::from(state.settings.flow_control_window());
  let new_requested = state.received_seq_nr + window;
  if (new_requested > state.requested_seq_nr || via_timeout)
    && let Some(pc) = state.producer_controller.clone()
  {
    state.requested_seq_nr = new_requested;
    let support_resend = !state.settings.only_flow_control();
    deferred.push(DeferredAction::SendToProducer(
      pc,
      ProducerControllerCommand::request(state.confirmed_seq_nr, state.requested_seq_nr, support_resend, via_timeout),
    ));
  }
}

fn execute_deferred<A>(actions: Vec<DeferredAction<A>>)
where
  A: Clone + Send + Sync + 'static, {
  for action in actions {
    match action {
      | DeferredAction::SendToProducer(mut target, msg) => if let Err(_error) = target.try_tell(msg) {},
      | DeferredAction::Deliver(mut target, msg) => if let Err(_error) = target.try_tell(msg) {},
    }
  }
}
