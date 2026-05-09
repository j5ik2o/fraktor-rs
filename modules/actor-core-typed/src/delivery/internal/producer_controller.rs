//! Internal implementation of the point-to-point reliable delivery producer controller.

#[cfg(test)]
mod tests;

use alloc::{string::String, vec::Vec};

use fraktor_actor_core_kernel_rs::event::logging::LogLevel;
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedLock};

use crate::{
  TypedActorRef,
  actor::TypedActorContext,
  behavior::Behavior,
  delivery::{
    ConsumerControllerCommand, DurableProducerQueueCommand, DurableProducerQueueState, MessageSent, NO_QUALIFIER,
    ProducerControllerCommand, ProducerControllerConfig, ProducerControllerRequestNext, SeqNr, SequencedMessage,
    StoreMessageSentAck, producer_controller_command::ProducerControllerCommandKind,
  },
  dsl::Behaviors,
};

/// Deferred side-effects executed after releasing the state lock.
///
/// Prevents re-entrant deadlock when `tell()` routes a message back
/// to the same actor via a message adapter.
pub(crate) enum DeferredAction<A>
where
  A: Clone + Send + Sync + 'static, {
  RequestNext(TypedActorRef<ProducerControllerRequestNext<A>>, ProducerControllerRequestNext<A>),
  SendSequenced(TypedActorRef<ConsumerControllerCommand<A>>, ConsumerControllerCommand<A>),
  TellDurableQueue {
    target:  TypedActorRef<DurableProducerQueueCommand<A>>,
    message: DurableProducerQueueCommand<A>,
    timeout: Option<DurableQueueTimeout>,
  },
}

#[derive(Clone, Copy)]
pub(crate) enum DurableQueueTimeout {
  Load { attempt: u32 },
  Store { seq_nr: SeqNr, attempt: u32 },
}

pub(crate) struct PendingDurableDelivery<A>
where
  A: Clone + Send + Sync + 'static, {
  sequenced:         SequencedMessage<A>,
  track_unconfirmed: bool,
}

pub(crate) struct ProducerControllerState<A>
where
  A: Clone + Send + Sync + 'static, {
  pub(crate) producer_id:         String,
  pub(crate) current_seq_nr:      SeqNr,
  pub(crate) confirmed_seq_nr:    SeqNr,
  pub(crate) requested_seq_nr:    SeqNr,
  pub(crate) requested:           bool,
  pub(crate) support_resend:      bool,
  /// Whether a `RequestNext` has been sent and we are awaiting a `Msg` response.
  /// Guards against infinite inline-dispatch loops (PC -> producer -> PC -> CC -> PC).
  pub(crate) awaiting_msg:        bool,
  /// Marks that the next outgoing message should carry `first=true`.
  /// Set when a new consumer registers so the CC can reset its state.
  pub(crate) send_first:          bool,
  pub(crate) unconfirmed:         Vec<SequencedMessage<A>>,
  pub(crate) producer:            Option<TypedActorRef<ProducerControllerRequestNext<A>>>,
  pub(crate) consumer_controller: Option<TypedActorRef<ConsumerControllerCommand<A>>>,
  pub(crate) send_adapter:        Option<TypedActorRef<A>>,
  pub(crate) durable_queue:       Option<TypedActorRef<DurableProducerQueueCommand<A>>>,
  pub(crate) store_ack_adapter:   Option<TypedActorRef<StoreMessageSentAck>>,
  pub(crate) pending_delivery:    Option<PendingDurableDelivery<A>>,
  pub(crate) awaiting_load:       bool,
  pub(crate) resend_first_seq_nr: Option<SeqNr>,
}

impl<A> ProducerControllerState<A>
where
  A: Clone + Send + Sync + 'static,
{
  pub(crate) const fn new(producer_id: String) -> Self {
    Self {
      producer_id,
      current_seq_nr: 1,
      confirmed_seq_nr: 0,
      requested_seq_nr: 0,
      requested: false,
      support_resend: true,
      awaiting_msg: false,
      send_first: true,
      unconfirmed: Vec::new(),
      producer: None,
      consumer_controller: None,
      send_adapter: None,
      durable_queue: None,
      store_ack_adapter: None,
      pending_delivery: None,
      awaiting_load: false,
      resend_first_seq_nr: None,
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
    Self::behavior_with_settings(producer_id, &ProducerControllerConfig::new(), None)
  }

  /// Creates the producer controller behavior with an optional durable
  /// queue for crash recovery.
  ///
  /// When `durable_queue` is `Some`, the controller will:
  /// - Send `LoadState` on startup to recover unconfirmed messages
  /// - Send `StoreMessageSent` before delivering each message
  /// - Send `StoreMessageConfirmed` when a confirmation is received
  ///
  /// When `durable_queue` is `None`, the behavior is identical to
  /// [`behavior`](Self::behavior).
  ///
  /// Corresponds to Pekko's `ProducerController.apply` with
  /// `durableQueueBehavior` parameter.
  #[must_use]
  pub fn behavior_with_durable_queue<A>(
    producer_id: impl Into<String>,
    durable_queue: Option<Behavior<DurableProducerQueueCommand<A>>>,
  ) -> Behavior<ProducerControllerCommand<A>>
  where
    A: Clone + Send + Sync + 'static, {
    Self::behavior_with_settings(producer_id, &ProducerControllerConfig::new(), durable_queue)
  }

  /// Creates the producer controller behavior with custom settings.
  #[must_use]
  pub fn behavior_with_settings<A>(
    producer_id: impl Into<String>,
    settings: &ProducerControllerConfig,
    durable_queue_behavior: Option<Behavior<DurableProducerQueueCommand<A>>>,
  ) -> Behavior<ProducerControllerCommand<A>>
  where
    A: Clone + Send + Sync + 'static, {
    let producer_id = producer_id.into();
    let settings = settings.clone();

    Behaviors::setup(move |ctx| {
      let self_ref = ctx.self_ref();
      if settings.chunk_large_messages_bytes() > 0 {
        let message = alloc::format!(
          "ProducerController does not support chunk_large_messages_bytes={}",
          settings.chunk_large_messages_bytes()
        );
        ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
        return Behaviors::stopped();
      }

      // メッセージアダプタを作成: A → ProducerControllerCommand::Msg
      let send_adapter = match ctx.message_adapter(|a: A| Ok(ProducerControllerCommand::msg(a))) {
        | Ok(adapter) => adapter,
        | Err(error) => {
          let message = alloc::format!("ProducerController failed to create send adapter: {:?}", error);
          ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
          return Behaviors::stopped();
        },
      };

      let state =
        SharedLock::new_with_driver::<DefaultMutex<_>>(ProducerControllerState::<A>::new(producer_id.clone()));
      let load_state_adapter = if durable_queue_behavior.is_some() {
        match ctx.message_adapter(|loaded: DurableProducerQueueState<A>| {
          Ok(ProducerControllerCommand::durable_queue_loaded(loaded))
        }) {
          | Ok(adapter) => Some(adapter),
          | Err(error) => {
            let message = alloc::format!("ProducerController failed to create durable queue load adapter: {:?}", error);
            ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
            return Behaviors::stopped();
          },
        }
      } else {
        None
      };
      let store_ack_adapter = if durable_queue_behavior.is_some() {
        match ctx
          .message_adapter(|ack: StoreMessageSentAck| Ok(ProducerControllerCommand::durable_queue_message_stored(ack)))
        {
          | Ok(adapter) => Some(adapter),
          | Err(error) => {
            let message = alloc::format!("ProducerController failed to create durable queue ack adapter: {:?}", error);
            ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
            return Behaviors::stopped();
          },
        }
      } else {
        None
      };
      let durable_queue = if let Some(durable_queue_behavior) = durable_queue_behavior.clone() {
        match ctx.spawn_anonymous(&durable_queue_behavior) {
          | Ok(child) => Some(child.into_actor_ref()),
          | Err(error) => {
            let message = alloc::format!("ProducerController failed to spawn durable queue: {:?}", error);
            ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
            return Behaviors::stopped();
          },
        }
      } else {
        None
      };
      state.with_lock(|s| {
        s.send_adapter = Some(send_adapter);
        s.store_ack_adapter = store_ack_adapter;
        s.awaiting_load = durable_queue.is_some();
        s.durable_queue = durable_queue.clone();
      });
      if let (Some(mut durable_queue), Some(load_state_adapter)) =
        (durable_queue.as_ref().cloned(), load_state_adapter.as_ref().cloned())
        && let Err(error) = durable_queue.try_tell(DurableProducerQueueCommand::load_state(load_state_adapter))
      {
        let message = alloc::format!("ProducerController failed to request durable queue state: {:?}", error);
        ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
        return Behaviors::stopped();
      } else if state.with_lock(|state| state.awaiting_load)
        && let Err(error) = ctx.schedule_once(
          settings.durable_queue_request_timeout(),
          self_ref.clone(),
          ProducerControllerCommand::durable_queue_load_timed_out(1),
        )
      {
        let message = alloc::format!("ProducerController failed to schedule durable queue load timeout: {:?}", error);
        ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
      }

      let runtime_settings = settings.clone();
      let runtime_load_state_adapter = load_state_adapter;
      Behaviors::receive_message(move |ctx, command: &ProducerControllerCommand<A>| {
        let mut stop_self = None::<String>;
        // ロック保持中に遅延アクションを収集し、ロック解放後に実行する。
        // メッセージアダプタ経由の再入デッドロックを回避するため。
        let deferred = state.with_lock(|state| {
          let mut deferred = Vec::new();
          match command.kind() {
            | ProducerControllerCommandKind::Start { producer } => {
              state.producer = Some(producer.clone());
              collect_request_next(state, &mut deferred);
            },
            | ProducerControllerCommandKind::RegisterConsumer { consumer_controller } => {
              state.consumer_controller = Some(consumer_controller.clone());
              // 新しい CC を登録する際にセッション状態をリセットする。
              // 次のメッセージに first=true を付けて CC に状態リセットを促す。
              state.send_first = true;
              state.requested = false;
              state.requested_seq_nr = 0;
              state.awaiting_msg = false;
              collect_request_next(state, &mut deferred);
            },
            | ProducerControllerCommandKind::Msg { message } => {
              state.awaiting_msg = false;
              collect_on_msg(state, message.clone(), &self_ref, &mut deferred);
            },
            | ProducerControllerCommandKind::Request { confirmed_seq_nr, request_up_to_seq_nr, support_resend } => {
              let previous_confirmed_seq_nr = state.confirmed_seq_nr;
              state.support_resend = *support_resend;
              state.on_confirmed(*confirmed_seq_nr);
              collect_store_confirmed(state, previous_confirmed_seq_nr, state.confirmed_seq_nr, &mut deferred);
              state.requested_seq_nr = *request_up_to_seq_nr;
              state.requested = true;
              collect_request_next(state, &mut deferred);
            },
            | ProducerControllerCommandKind::Resend { from_seq_nr } => {
              collect_resend(state, *from_seq_nr, &mut deferred);
            },
            | ProducerControllerCommandKind::Ack { confirmed_seq_nr } => {
              let previous_confirmed_seq_nr = state.confirmed_seq_nr;
              state.on_confirmed(*confirmed_seq_nr);
              collect_store_confirmed(state, previous_confirmed_seq_nr, state.confirmed_seq_nr, &mut deferred);
            },
            | ProducerControllerCommandKind::DurableQueueLoaded { state: loaded } => {
              state.current_seq_nr = loaded.current_seq_nr();
              state.confirmed_seq_nr = loaded.highest_confirmed_seq_nr();
              state.unconfirmed = loaded
                .unconfirmed()
                .iter()
                .map(|sent| {
                  SequencedMessage::new(
                    state.producer_id.clone(),
                    sent.seq_nr(),
                    sent.message().clone(),
                    false,
                    sent.ack(),
                    self_ref.clone(),
                  )
                })
                .collect();
              state.awaiting_load = false;
              collect_request_next(state, &mut deferred);
            },
            | ProducerControllerCommandKind::DurableQueueMessageStored { ack } => {
              collect_on_durable_queue_message_stored(state, ack, &mut deferred);
            },
            | ProducerControllerCommandKind::DurableQueueLoadTimedOut { attempt } => {
              if state.awaiting_load {
                if *attempt >= runtime_settings.durable_queue_retry_attempts() {
                  stop_self =
                    Some(alloc::format!("ProducerController durable queue load timed out after {} attempts", attempt));
                } else if let (Some(durable_queue), Some(load_state_adapter)) =
                  (state.durable_queue.clone(), runtime_load_state_adapter.clone())
                {
                  deferred.push(DeferredAction::TellDurableQueue {
                    target:  durable_queue,
                    message: DurableProducerQueueCommand::load_state(load_state_adapter),
                    timeout: Some(DurableQueueTimeout::Load { attempt: attempt + 1 }),
                  });
                }
              }
            },
            | ProducerControllerCommandKind::DurableQueueStoreTimedOut { seq_nr, attempt } => {
              if let Some(pending_delivery) = state.pending_delivery.as_ref()
                && pending_delivery.sequenced.seq_nr() == *seq_nr
              {
                if *attempt >= runtime_settings.durable_queue_retry_attempts() {
                  stop_self = Some(alloc::format!(
                    "ProducerController durable queue store timed out for seq_nr {} after {} attempts",
                    seq_nr,
                    attempt
                  ));
                } else if let (Some(durable_queue), Some(store_ack_adapter)) =
                  (state.durable_queue.clone(), state.store_ack_adapter.clone())
                {
                  let sent = MessageSent::new(
                    pending_delivery.sequenced.seq_nr(),
                    pending_delivery.sequenced.message().clone(),
                    pending_delivery.sequenced.ack(),
                    NO_QUALIFIER,
                    0,
                  );
                  deferred.push(DeferredAction::TellDurableQueue {
                    target:  durable_queue,
                    message: DurableProducerQueueCommand::store_message_sent(sent, store_ack_adapter),
                    timeout: Some(DurableQueueTimeout::Store { seq_nr: *seq_nr, attempt: attempt + 1 }),
                  });
                }
              }
            },
            | ProducerControllerCommandKind::ResendFirstUnconfirmed { seq_nr } => {
              if state.resend_first_seq_nr == Some(*seq_nr) {
                state.resend_first_seq_nr = None;
              }
              if let Some(first) = state.unconfirmed.first().cloned()
                && first.seq_nr() == *seq_nr
                && let Some(cc) = state.consumer_controller.clone()
              {
                deferred
                  .push(DeferredAction::SendSequenced(cc, ConsumerControllerCommand::sequenced_msg(first.as_first())));
              }
            },
          }
          maybe_schedule_resend_first(state, &runtime_settings, &self_ref, ctx);
          deferred
        }); // ステートロックはここで解放される

        execute_deferred(ctx, deferred, &runtime_settings, &self_ref);
        if let Some(message) = stop_self {
          ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
          if let Err(error) = ctx.stop_self() {
            let stop_message = alloc::format!("ProducerController failed to stop after timeout: {:?}", error);
            ctx.system().emit_log(LogLevel::Warn, stop_message, Some(ctx.pid()), None);
          }
        }
        Ok(Behaviors::same())
      })
    })
  }
}

fn maybe_schedule_resend_first<A>(
  state: &mut ProducerControllerState<A>,
  settings: &ProducerControllerConfig,
  self_ref: &TypedActorRef<ProducerControllerCommand<A>>,
  ctx: &TypedActorContext<'_, ProducerControllerCommand<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  let Some(first_seq_nr) = state.unconfirmed.first().map(SequencedMessage::seq_nr) else {
    state.resend_first_seq_nr = None;
    return;
  };
  if state.consumer_controller.is_none() {
    state.resend_first_seq_nr = None;
    return;
  }
  if state.resend_first_seq_nr == Some(first_seq_nr) {
    return;
  }
  match ctx.schedule_once(
    settings.durable_queue_resend_first_interval(),
    self_ref.clone(),
    ProducerControllerCommand::resend_first_unconfirmed(first_seq_nr),
  ) {
    | Ok(_) => {
      state.resend_first_seq_nr = Some(first_seq_nr);
    },
    | Err(error) => {
      let message = alloc::format!(
        "ProducerController failed to schedule resend-first timer for seq_nr {}: {:?}",
        first_seq_nr,
        error
      );
      ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
      state.resend_first_seq_nr = None;
    },
  }
}

fn collect_request_next<A>(state: &mut ProducerControllerState<A>, deferred: &mut Vec<DeferredAction<A>>)
where
  A: Clone + Send + Sync + 'static, {
  if state.awaiting_load {
    return;
  }
  if state.awaiting_msg {
    return;
  }
  if state.producer.is_none() || state.consumer_controller.is_none() {
    return;
  }
  let allow_first = !state.requested && state.send_first;
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
    state.awaiting_msg = true;
  }
}

pub(crate) fn collect_on_msg<A>(
  state: &mut ProducerControllerState<A>,
  message: A,
  self_ref: &TypedActorRef<ProducerControllerCommand<A>>,
  deferred: &mut Vec<DeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  let seq_nr = state.current_seq_nr;
  let first = state.send_first;
  let sequenced = SequencedMessage::new(state.producer_id.clone(), seq_nr, message, first, false, self_ref.clone());
  if let (Some(durable_queue), Some(store_ack_adapter)) = (state.durable_queue.clone(), state.store_ack_adapter.clone())
  {
    let sent = MessageSent::new(seq_nr, sequenced.message().clone(), sequenced.ack(), NO_QUALIFIER, 0);
    state.pending_delivery = Some(PendingDurableDelivery { sequenced, track_unconfirmed: state.support_resend });
    deferred.push(DeferredAction::TellDurableQueue {
      target:  durable_queue,
      message: DurableProducerQueueCommand::store_message_sent(sent, store_ack_adapter),
      timeout: Some(DurableQueueTimeout::Store { seq_nr, attempt: 1 }),
    });
  } else {
    if first {
      state.send_first = false;
    }
    if state.support_resend {
      state.unconfirmed.push(sequenced.clone());
    }
    if let Some(cc) = state.consumer_controller.clone() {
      deferred.push(DeferredAction::SendSequenced(cc, ConsumerControllerCommand::sequenced_msg(sequenced)));
    }
  }

  state.current_seq_nr += 1;
  // ここで collect_request_next を呼ばない。インラインディスパッチでは CC の
  // Request が同じコールスタック内で処理され、awaiting_msg の設定→解除→再設定が
  // 1回のバッチ内で繰り返されて無限ループになる。デマンド補充は Request コマンド
  // 到着時にのみ行う。
}

fn collect_resend<A>(state: &ProducerControllerState<A>, from_seq_nr: SeqNr, deferred: &mut Vec<DeferredAction<A>>)
where
  A: Clone + Send + Sync + 'static, {
  if let Some(cc) = state.consumer_controller.clone() {
    // リセンドでは first フラグを設定しない。first=true は新セッション開始を
    // 意味し、CC がステートをリセットして既受信メッセージを消失させるため。
    for msg in state.unconfirmed.iter().filter(|msg| msg.seq_nr() >= from_seq_nr).cloned() {
      deferred.push(DeferredAction::SendSequenced(cc.clone(), ConsumerControllerCommand::sequenced_msg(msg)));
    }
  }
}

fn collect_store_confirmed<A>(
  state: &ProducerControllerState<A>,
  previous_confirmed_seq_nr: SeqNr,
  confirmed_seq_nr: SeqNr,
  deferred: &mut Vec<DeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  if confirmed_seq_nr <= previous_confirmed_seq_nr {
    return;
  }
  if let Some(durable_queue) = state.durable_queue.clone() {
    deferred.push(DeferredAction::TellDurableQueue {
      target:  durable_queue,
      message: DurableProducerQueueCommand::store_message_confirmed(confirmed_seq_nr, NO_QUALIFIER, 0),
      timeout: None,
    });
  }
}

pub(crate) fn collect_on_durable_queue_message_stored<A>(
  state: &mut ProducerControllerState<A>,
  ack: &StoreMessageSentAck,
  deferred: &mut Vec<DeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  let Some(pending_delivery) = state.pending_delivery.take() else {
    return;
  };
  if pending_delivery.sequenced.seq_nr() != ack.stored_seq_nr() {
    state.pending_delivery = Some(pending_delivery);
    return;
  }
  if pending_delivery.sequenced.first() {
    state.send_first = false;
  }
  if pending_delivery.track_unconfirmed {
    state.unconfirmed.push(pending_delivery.sequenced.clone());
  }
  if let Some(cc) = state.consumer_controller.clone() {
    deferred
      .push(DeferredAction::SendSequenced(cc, ConsumerControllerCommand::sequenced_msg(pending_delivery.sequenced)));
  }
}

fn execute_deferred<A>(
  ctx: &mut TypedActorContext<'_, ProducerControllerCommand<A>>,
  actions: Vec<DeferredAction<A>>,
  settings: &ProducerControllerConfig,
  self_ref: &TypedActorRef<ProducerControllerCommand<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  for action in actions {
    match action {
      | DeferredAction::RequestNext(mut target, msg) => {
        if let Err(error) = target.try_tell(msg) {
          let message = alloc::format!("ProducerController failed to request next message: {:?}", error);
          ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
        }
      },
      | DeferredAction::SendSequenced(mut target, msg) => {
        if let Err(error) = target.try_tell(msg) {
          let message = alloc::format!("ProducerController failed to send sequenced message: {:?}", error);
          ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
        }
      },
      | DeferredAction::TellDurableQueue { mut target, message, timeout } => {
        if let Err(error) = target.try_tell(message) {
          let message = alloc::format!("ProducerController failed to talk to durable queue: {:?}", error);
          ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
          if let Err(stop_error) = ctx.stop_self() {
            let stop_message =
              alloc::format!("ProducerController failed to stop after durable queue failure: {:?}", stop_error);
            ctx.system().emit_log(LogLevel::Warn, stop_message, Some(ctx.pid()), None);
          }
        } else if let Some(timeout) = timeout {
          let command = match timeout {
            | DurableQueueTimeout::Load { attempt } => ProducerControllerCommand::durable_queue_load_timed_out(attempt),
            | DurableQueueTimeout::Store { seq_nr, attempt } => {
              ProducerControllerCommand::durable_queue_store_timed_out(seq_nr, attempt)
            },
          };
          if let Err(error) = ctx.schedule_once(settings.durable_queue_request_timeout(), self_ref.clone(), command) {
            let message = alloc::format!("ProducerController failed to schedule durable queue timeout: {:?}", error);
            ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
          }
        }
      },
    }
  }
}
