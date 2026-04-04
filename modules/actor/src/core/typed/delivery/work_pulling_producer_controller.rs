//! Work-pulling producer controller for reliable delivery across multiple workers.

#[cfg(test)]
mod tests;

use alloc::{
  collections::{BTreeMap, BTreeSet, VecDeque},
  string::{String, ToString},
  vec::Vec,
};

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::core::{
  kernel::{
    actor::{
      actor_ref::ActorRef,
      error::SendError,
      messaging::{AnyMessage, system_message::SystemMessage},
    },
    event::logging::LogLevel,
  },
  typed::{
    TypedActorRef,
    actor::TypedActorContext,
    behavior::Behavior,
    delivery::{
      ConsumerControllerCommand, DurableProducerQueueCommand, DurableProducerQueueState, MessageSent,
      ProducerController, ProducerControllerCommand, ProducerControllerRequestNext, ProducerControllerSettings,
      StoreMessageSentAck, WorkPullingProducerControllerCommand, WorkPullingProducerControllerRequestNext,
      WorkPullingProducerControllerSettings, WorkerStats,
      work_pulling_producer_controller_command::WorkPullingProducerControllerCommandKind,
    },
    dsl::Behaviors,
    props::TypedProps,
    receptionist::{Listing, Receptionist, ServiceKey},
  },
};

/// Converts a `Pid` to a `u64` key suitable for `BTreeMap`.
///
/// Uses the `value` field of `Pid` as the key. In the same actor system
/// this is unique for live actors.
const fn pid_key(actor_ref: &ActorRef) -> u64 {
  actor_ref.pid().value()
}

/// Deferred side-effects executed after releasing the state lock.
enum WppcDeferredAction<A>
where
  A: Clone + Send + Sync + 'static, {
  TellWorker {
    target:              TypedActorRef<ProducerControllerCommand<A>>,
    message:             ProducerControllerCommand<A>,
    worker_key:          u64,
    worker_local_seq_nr: u64,
  },
  TellDurableQueue {
    target:  TypedActorRef<DurableProducerQueueCommand<A>>,
    message: DurableProducerQueueCommand<A>,
    timeout: Option<WppcDurableQueueTimeout>,
  },
  TellSelf(TypedActorRef<WorkPullingProducerControllerCommand<A>>, WorkPullingProducerControllerCommand<A>),
  TellWorkerStats(TypedActorRef<WorkerStats>, WorkerStats),
  RequestNext(TypedActorRef<WorkPullingProducerControllerRequestNext<A>>, WorkPullingProducerControllerRequestNext<A>),
  /// Stop a per-worker `ProducerController` that is no longer needed.
  StopWorkerPc(TypedActorRef<ProducerControllerCommand<A>>),
  /// Spawn a per-worker ProducerController and wire it up.
  SpawnWorker {
    worker_ref:                   ActorRef,
    producer_id:                  String,
    demand_adapter:               TypedActorRef<ProducerControllerRequestNext<A>>,
    producer_controller_settings: ProducerControllerSettings,
  },
  LogDropped {
    total_seq_nr: u64,
    buffered_len: usize,
    buffer_size:  u32,
    message_type: &'static str,
  },
}

#[derive(Clone, Copy)]
enum WppcDurableQueueTimeout {
  Load { attempt: u32 },
  Store { seq_nr: u64, attempt: u32 },
}

#[derive(Clone)]
enum BufferedWork<A>
where
  A: Clone + Send + Sync + 'static, {
  Fresh { seq_nr: u64, message: A },
  Replay(MessageSent<A>),
}

struct PendingDurableStore<A>
where
  A: Clone + Send + Sync + 'static, {
  message:                A,
  worker_key:             u64,
  worker_local_seq_nr:    u64,
  confirmation_qualifier: String,
  replay_confirmation_of: Option<(String, u64)>,
}

/// Tracks a single worker and its associated `ProducerController`.
struct WorkerEntry<A>
where
  A: Clone + Send + Sync + 'static, {
  /// The per-worker producer identifier used as confirmation qualifier.
  producer_id:         String,
  /// The per-worker `ProducerController` ref.
  producer_controller: TypedActorRef<ProducerControllerCommand<A>>,
  /// The next per-worker sequence number announced by the `ProducerController`.
  next_seq_nr:         u64,
  /// The highest per-worker sequence number already confirmed.
  confirmed_seq_nr:    u64,
  /// Mapping from per-worker sequence number to sent messages awaiting confirmation.
  in_flight:           BTreeMap<u64, MessageSent<A>>,
  /// Whether this worker has pending demand.
  has_demand:          bool,
}

/// Internal state for the work-pulling producer controller.
struct WorkPullingState<A>
where
  A: Clone + Send + Sync + 'static, {
  producer_id:       String,
  current_seq_nr:    u64,
  producer:          Option<TypedActorRef<WorkPullingProducerControllerRequestNext<A>>>,
  send_adapter:      Option<TypedActorRef<A>>,
  /// Adapter that per-worker PCs use to signal demand back to the WPPC.
  demand_adapter:    Option<TypedActorRef<ProducerControllerRequestNext<A>>>,
  /// Workers keyed by `Pid::value()`.
  workers:           BTreeMap<u64, WorkerEntry<A>>,
  /// Round-robin index for distributing messages.
  next_worker:       usize,
  /// Buffered messages when no worker has demand.
  buffered:          VecDeque<BufferedWork<A>>,
  /// Maximum buffer size.
  buffer_size:       u32,
  /// Whether we have sent a RequestNext to the producer and are awaiting a Msg.
  awaiting_msg:      bool,
  /// Whether this controller is still waiting for its durable queue state.
  awaiting_load:     bool,
  /// Optional durable queue child owned by this controller.
  durable_queue:     Option<TypedActorRef<DurableProducerQueueCommand<A>>>,
  /// Adapter used for `StoreMessageSentAck`.
  store_ack_adapter: Option<TypedActorRef<StoreMessageSentAck>>,
  /// Messages persisted to the durable queue but not yet handed to a worker.
  pending_stores:    BTreeMap<u64, PendingDurableStore<A>>,
}

impl<A> WorkPullingState<A>
where
  A: Clone + Send + Sync + 'static,
{
  const fn new(producer_id: String, buffer_size: u32) -> Self {
    Self {
      producer_id,
      current_seq_nr: 1,
      producer: None,
      send_adapter: None,
      demand_adapter: None,
      workers: BTreeMap::new(),
      next_worker: 0,
      buffered: VecDeque::new(),
      buffer_size,
      awaiting_msg: false,
      awaiting_load: false,
      durable_queue: None,
      store_ack_adapter: None,
      pending_stores: BTreeMap::new(),
    }
  }

  /// Returns the number of registered workers.
  fn worker_count(&self) -> u32 {
    self.workers.len() as u32
  }

  /// Finds a worker key that has demand, using round-robin starting from
  /// `next_worker`.
  fn find_worker_with_demand(&mut self) -> Option<u64> {
    let keys: Vec<u64> = self.workers.keys().copied().collect();
    if keys.is_empty() {
      return None;
    }
    let len = keys.len();
    for i in 0..len {
      let idx = (self.next_worker + i) % len;
      let key = keys[idx];
      if let Some(entry) = self.workers.get(&key)
        && entry.has_demand
      {
        self.next_worker = (idx + 1) % len;
        return Some(key);
      }
    }
    None
  }

  /// Returns true if any worker has demand.
  fn any_worker_has_demand(&self) -> bool {
    self.workers.values().any(|w| w.has_demand)
  }
}

/// Factory for creating a `WorkPullingProducerController` behavior.
///
/// The `WorkPullingProducerController` implements the work-pulling pattern
/// where multiple worker actors (consumers) pull tasks at their own pace.
/// Workers register dynamically via a
/// [`ServiceKey`](crate::core::typed::receptionist::ServiceKey) and the
/// [`Receptionist`](crate::core::typed::receptionist::Receptionist).
///
/// Each registered worker gets its own internal `ProducerController` for
/// flow-controlled, sequence-numbered delivery.
pub struct WorkPullingProducerController;

impl WorkPullingProducerController {
  /// Creates a `Start` command.
  #[must_use]
  pub const fn start<A>(
    producer: TypedActorRef<WorkPullingProducerControllerRequestNext<A>>,
  ) -> WorkPullingProducerControllerCommand<A>
  where
    A: Clone + Send + Sync + 'static, {
    WorkPullingProducerControllerCommand::start(producer)
  }

  /// Creates a `GetWorkerStats` command.
  #[must_use]
  pub const fn get_worker_stats<A>(reply_to: TypedActorRef<WorkerStats>) -> WorkPullingProducerControllerCommand<A>
  where
    A: Clone + Send + Sync + 'static, {
    WorkPullingProducerControllerCommand::get_worker_stats(reply_to)
  }

  /// Creates the work-pulling producer controller behavior with an optional
  /// durable queue for crash recovery.
  ///
  /// When `durable_queue` is `Some`, this controller owns the durable queue as
  /// a child actor and loads its state before requesting new work.
  ///
  /// When `durable_queue` is `None`, the behavior is identical to
  /// [`behavior`](Self::behavior).
  ///
  /// Corresponds to Pekko's `WorkPullingProducerController.apply` with
  /// `durableQueueBehavior` parameter.
  #[must_use]
  pub fn behavior_with_durable_queue<A>(
    producer_id: impl Into<String>,
    worker_service_key: ServiceKey<ConsumerControllerCommand<A>>,
    durable_queue: Option<Behavior<super::DurableProducerQueueCommand<A>>>,
  ) -> Behavior<WorkPullingProducerControllerCommand<A>>
  where
    A: Clone + Send + Sync + 'static, {
    Self::behavior_with_settings(
      producer_id,
      worker_service_key,
      &WorkPullingProducerControllerSettings::new(),
      durable_queue,
    )
  }

  /// Creates the work-pulling producer controller behavior with default
  /// settings.
  #[must_use]
  pub fn behavior<A>(
    producer_id: impl Into<String>,
    worker_service_key: ServiceKey<ConsumerControllerCommand<A>>,
  ) -> Behavior<WorkPullingProducerControllerCommand<A>>
  where
    A: Clone + Send + Sync + 'static, {
    Self::behavior_with_settings(producer_id, worker_service_key, &WorkPullingProducerControllerSettings::new(), None)
  }

  /// Creates the work-pulling producer controller behavior with custom
  /// settings.
  #[must_use]
  pub fn behavior_with_settings<A>(
    producer_id: impl Into<String>,
    worker_service_key: ServiceKey<ConsumerControllerCommand<A>>,
    settings: &WorkPullingProducerControllerSettings,
    durable_queue_behavior: Option<Behavior<DurableProducerQueueCommand<A>>>,
  ) -> Behavior<WorkPullingProducerControllerCommand<A>>
  where
    A: Clone + Send + Sync + 'static, {
    let producer_id = producer_id.into();
    let buffer_size = settings.buffer_size();
    let internal_ask_timeout = settings.internal_ask_timeout();
    let producer_controller_settings = settings.producer_controller_settings().clone();
    let durable_queue_request_timeout = producer_controller_settings.durable_queue_request_timeout();

    Behaviors::setup(move |ctx| {
      let self_ref = ctx.self_ref();
      if producer_controller_settings.chunk_large_messages_bytes() > 0 {
        let message = alloc::format!(
          "WorkPullingProducerController does not support chunk_large_messages_bytes={}",
          producer_controller_settings.chunk_large_messages_bytes()
        );
        ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
        return Behaviors::stopped();
      }

      // メッセージアダプタを作成: A → WorkPullingProducerControllerCommand::Msg
      let send_adapter = match ctx.message_adapter(|a: A| Ok(WorkPullingProducerControllerCommand::msg(a))) {
        | Ok(adapter) => adapter,
        | Err(error) => {
          let message = alloc::format!("WorkPullingProducerController failed to create send adapter: {:?}", error);
          ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
          return Behaviors::stopped();
        },
      };

      // メッセージアダプタを作成: ProducerControllerRequestNext<A> →
      // WorkPullingProducerControllerCommand::InternalDemand
      let demand_adapter = match ctx.message_adapter(|req: ProducerControllerRequestNext<A>| {
        Ok(WorkPullingProducerControllerCommand::internal_demand(req))
      }) {
        | Ok(adapter) => adapter,
        | Err(error) => {
          let message = alloc::format!("WorkPullingProducerController failed to create demand adapter: {:?}", error);
          ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
          return Behaviors::stopped();
        },
      };

      // メッセージアダプタを作成: Listing →
      // WorkPullingProducerControllerCommand::WorkerListing
      let listing_adapter = match ctx
        .message_adapter(|listing: Listing| Ok(WorkPullingProducerControllerCommand::worker_listing(listing)))
      {
        | Ok(adapter) => adapter,
        | Err(error) => {
          let message = alloc::format!("WorkPullingProducerController failed to create listing adapter: {:?}", error);
          ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
          return Behaviors::stopped();
        },
      };

      // ワーカー検出のために Receptionist をサブスクライブする。
      subscribe_to_receptionist(ctx, &worker_service_key, &listing_adapter);

      let state = ArcShared::new(RuntimeMutex::new(WorkPullingState::<A>::new(producer_id.clone(), buffer_size)));
      let load_state_adapter = if durable_queue_behavior.is_some() {
        match ctx.message_adapter(|loaded: DurableProducerQueueState<A>| {
          Ok(WorkPullingProducerControllerCommand::durable_queue_loaded(loaded))
        }) {
          | Ok(adapter) => Some(adapter),
          | Err(error) => {
            let message =
              alloc::format!("WorkPullingProducerController failed to create durable queue load adapter: {:?}", error);
            ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
            return Behaviors::stopped();
          },
        }
      } else {
        None
      };
      let store_ack_adapter = if durable_queue_behavior.is_some() {
        match ctx.message_adapter(|ack: StoreMessageSentAck| {
          Ok(WorkPullingProducerControllerCommand::durable_queue_message_stored(ack))
        }) {
          | Ok(adapter) => Some(adapter),
          | Err(error) => {
            let message =
              alloc::format!("WorkPullingProducerController failed to create durable queue ack adapter: {:?}", error);
            ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
            return Behaviors::stopped();
          },
        }
      } else {
        None
      };
      let durable_queue = if let Some(durable_queue_behavior) = durable_queue_behavior.as_ref() {
        match ctx.spawn_anonymous(&durable_queue_behavior.clone()) {
          | Ok(child) => Some(child.into_actor_ref()),
          | Err(error) => {
            let message = alloc::format!("WorkPullingProducerController failed to spawn durable queue: {:?}", error);
            ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
            return Behaviors::stopped();
          },
        }
      } else {
        None
      };
      {
        let mut s = state.lock();
        s.send_adapter = Some(send_adapter);
        s.demand_adapter = Some(demand_adapter);
        s.store_ack_adapter = store_ack_adapter;
        s.durable_queue = durable_queue.clone();
        s.awaiting_load = durable_queue.is_some();
      }
      if let (Some(mut durable_queue), Some(load_state_adapter)) =
        (durable_queue.as_ref().cloned(), load_state_adapter.as_ref().cloned())
        && let Err(error) = durable_queue.try_tell(DurableProducerQueueCommand::load_state(load_state_adapter))
      {
        let message =
          alloc::format!("WorkPullingProducerController failed to request durable queue state: {:?}", error);
        ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
        return Behaviors::stopped();
      } else if state.lock().awaiting_load
        && let Err(error) = ctx.schedule_once(
          durable_queue_request_timeout,
          self_ref.clone(),
          WorkPullingProducerControllerCommand::durable_queue_load_timed_out(1),
        )
      {
        let message =
          alloc::format!("WorkPullingProducerController failed to schedule durable queue load timeout: {:?}", error);
        ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
      }

      let producer_id_inner = producer_id.clone();
      let runtime_load_state_adapter = load_state_adapter;
      let runtime_producer_controller_settings = producer_controller_settings.clone();
      let runtime_durable_queue_request_timeout = durable_queue_request_timeout;
      Behaviors::receive_message(move |ctx, command: &WorkPullingProducerControllerCommand<A>| {
        let mut stop_self = None::<String>;
        let mut timeout_warning = None::<String>;
        let deferred = {
          let mut state = state.lock();
          let mut deferred: Vec<WppcDeferredAction<A>> = Vec::new();
          match command.kind() {
            | WorkPullingProducerControllerCommandKind::Start { producer } => {
              state.producer = Some(producer.clone());
              collect_maybe_request_next(&mut state, &mut deferred);
            },
            | WorkPullingProducerControllerCommandKind::Msg { message } => {
              state.awaiting_msg = false;
              collect_on_msg(&mut state, message.clone(), &mut deferred);
            },
            | WorkPullingProducerControllerCommandKind::GetWorkerStats { reply_to } => {
              let stats = WorkerStats::new(state.worker_count());
              deferred.push(WppcDeferredAction::TellWorkerStats(reply_to.clone(), stats));
            },
            | WorkPullingProducerControllerCommandKind::WorkerListing { listing } => {
              collect_on_worker_listing(
                &mut state,
                listing,
                &producer_id_inner,
                &self_ref,
                &runtime_producer_controller_settings,
                &mut deferred,
              );
            },
            | WorkPullingProducerControllerCommandKind::InternalDemand { request } => {
              collect_on_internal_demand(&mut state, request, &mut deferred);
            },
            | WorkPullingProducerControllerCommandKind::DurableQueueLoaded { state: loaded } => {
              state.current_seq_nr = loaded.current_seq_nr();
              state.awaiting_load = false;
              for sent in loaded.unconfirmed() {
                deferred.push(WppcDeferredAction::TellSelf(
                  self_ref.clone(),
                  WorkPullingProducerControllerCommand::replay_stored_message(sent.clone()),
                ));
              }
              if loaded.unconfirmed().is_empty() {
                collect_maybe_request_next(&mut state, &mut deferred);
              }
            },
            | WorkPullingProducerControllerCommandKind::DurableQueueMessageStored { ack } => {
              collect_on_durable_queue_message_stored(&mut state, ack, &self_ref, &mut deferred);
            },
            | WorkPullingProducerControllerCommandKind::DurableQueueLoadTimedOut { attempt } => {
              if state.awaiting_load {
                if *attempt >= runtime_producer_controller_settings.durable_queue_retry_attempts() {
                  stop_self = Some(alloc::format!(
                    "WorkPullingProducerController durable queue load timed out after {} attempts",
                    attempt
                  ));
                } else if let (Some(durable_queue), Some(load_state_adapter)) =
                  (state.durable_queue.clone(), runtime_load_state_adapter.clone())
                {
                  deferred.push(WppcDeferredAction::TellDurableQueue {
                    target:  durable_queue,
                    message: DurableProducerQueueCommand::load_state(load_state_adapter),
                    timeout: Some(WppcDurableQueueTimeout::Load { attempt: attempt + 1 }),
                  });
                }
              }
            },
            | WorkPullingProducerControllerCommandKind::DurableQueueStoreTimedOut { seq_nr, attempt } => {
              if let Some(pending) = state.pending_stores.get(seq_nr) {
                if *attempt >= runtime_producer_controller_settings.durable_queue_retry_attempts() {
                  stop_self = Some(alloc::format!(
                    "WorkPullingProducerController durable queue store timed out for seq_nr {} after {} attempts",
                    seq_nr,
                    attempt
                  ));
                } else if let (Some(durable_queue), Some(store_ack_adapter)) =
                  (state.durable_queue.clone(), state.store_ack_adapter.clone())
                {
                  let sent = MessageSent::new(
                    *seq_nr,
                    pending.message.clone(),
                    false,
                    pending.confirmation_qualifier.clone(),
                    0,
                  );
                  deferred.push(WppcDeferredAction::TellDurableQueue {
                    target:  durable_queue,
                    message: DurableProducerQueueCommand::store_message_sent(sent, store_ack_adapter),
                    timeout: Some(WppcDurableQueueTimeout::Store { seq_nr: *seq_nr, attempt: attempt + 1 }),
                  });
                }
              }
            },
            | WorkPullingProducerControllerCommandKind::WorkerDeliveryTimedOut { worker_key, worker_local_seq_nr } => {
              if let Some(entry) = state.workers.remove(worker_key) {
                if let Some(sent) = entry.in_flight.get(worker_local_seq_nr) {
                  timeout_warning = Some(alloc::format!(
                    "WorkPullingProducerController worker delivery timed out for worker {} local_seq_nr {} total_seq_nr {}; removing stalled worker and replaying in-flight messages",
                    worker_key,
                    worker_local_seq_nr,
                    sent.seq_nr()
                  ));
                }
                deferred.push(WppcDeferredAction::StopWorkerPc(entry.producer_controller.clone()));
                for inflight in entry.in_flight.into_values() {
                  deferred.push(WppcDeferredAction::TellSelf(
                    self_ref.clone(),
                    WorkPullingProducerControllerCommand::replay_stored_message(inflight),
                  ));
                }
                collect_maybe_request_next(&mut state, &mut deferred);
              }
            },
            | WorkPullingProducerControllerCommandKind::ReplayStoredMessage { sent } => {
              collect_on_replayed_message(&mut state, sent.clone(), &mut deferred);
            },
          }
          deferred
        }; // ステートロックはここで解放される

        execute_wppc_deferred(deferred, ctx, &state, internal_ask_timeout, runtime_durable_queue_request_timeout);
        if let Some(message) = timeout_warning {
          ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
        }
        if let Some(message) = stop_self {
          ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
          if let Err(error) = ctx.stop_self() {
            let stop_message =
              alloc::format!("WorkPullingProducerController failed to stop after timeout: {:?}", error);
            ctx.system().emit_log(LogLevel::Warn, stop_message, Some(ctx.pid()), None);
          }
        }
        Ok(Behaviors::same())
      })
    })
  }
}

/// Subscribes to the Receptionist for a specific worker service key.
fn subscribe_to_receptionist<A>(
  ctx: &mut TypedActorContext<'_, WorkPullingProducerControllerCommand<A>>,
  worker_service_key: &ServiceKey<ConsumerControllerCommand<A>>,
  listing_adapter: &TypedActorRef<Listing>,
) where
  A: Clone + Send + Sync + 'static, {
  let subscribe_cmd = Receptionist::subscribe(worker_service_key, listing_adapter.clone());
  let system = ctx.system();
  if let Some(mut receptionist_ref) = system.receptionist_ref() {
    if let Err(error) = receptionist_ref.try_tell(subscribe_cmd) {
      ctx.system().emit_log(
        LogLevel::Warn,
        alloc::format!("work-pulling producer controller failed to subscribe receptionist: {:?}", error),
        Some(ctx.pid()),
        None,
      );
    }
  } else {
    ctx.system().emit_log(
      LogLevel::Warn,
      "work-pulling producer controller skipped receptionist subscription because no receptionist is installed",
      Some(ctx.pid()),
      None,
    );
  }
}

/// Collects deferred actions for a worker listing update.
fn collect_on_worker_listing<A>(
  state: &mut WorkPullingState<A>,
  listing: &Listing,
  producer_id: &str,
  self_ref: &TypedActorRef<WorkPullingProducerControllerCommand<A>>,
  producer_controller_settings: &ProducerControllerSettings,
  deferred: &mut Vec<WppcDeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  let current_keys: BTreeSet<u64> = listing.refs().iter().map(pid_key).collect();

  // リスティングに存在しなくなったワーカーを削除する。
  let removed_keys: Vec<u64> = state.workers.keys().filter(|k| !current_keys.contains(k)).copied().collect();

  for key in &removed_keys {
    if let Some(entry) = state.workers.remove(key) {
      let unconfirmed: Vec<MessageSent<A>> = entry.in_flight.into_values().collect();
      // 削除されたワーカーの per-worker PC を停止する（アクターリーク防止）
      deferred.push(WppcDeferredAction::StopWorkerPc(entry.producer_controller));
      for sent in unconfirmed {
        deferred.push(WppcDeferredAction::TellSelf(
          self_ref.clone(),
          WorkPullingProducerControllerCommand::replay_stored_message(sent),
        ));
      }
    }
  }

  // 新規ワーカーの生成をスケジュールする（ロック外で実行）。
  if let Some(demand_adapter) = state.demand_adapter.clone() {
    for actor_ref in listing.refs() {
      let key = pid_key(actor_ref);
      if state.workers.contains_key(&key) {
        continue;
      }

      let pc_producer_id = alloc::format!("{}-worker-{}", producer_id, actor_ref.pid());

      deferred.push(WppcDeferredAction::SpawnWorker {
        worker_ref:                   actor_ref.clone(),
        producer_id:                  pc_producer_id,
        demand_adapter:               demand_adapter.clone(),
        producer_controller_settings: producer_controller_settings.clone(),
      });
    }
  }

  // バッファ済みメッセージをデマンドのあるワーカーに排出する。
  collect_drain_buffered(state, deferred);
  collect_maybe_request_next(state, deferred);
}

/// Handles a message from the producer (within lock).
fn collect_on_msg<A>(state: &mut WorkPullingState<A>, message: A, deferred: &mut Vec<WppcDeferredAction<A>>)
where
  A: Clone + Send + Sync + 'static, {
  let total_seq_nr = state.current_seq_nr;
  let work = BufferedWork::Fresh { seq_nr: total_seq_nr, message };
  if let Some(worker_key) = state.find_worker_with_demand() {
    collect_send_to_worker(state, worker_key, work, deferred);
  } else if (state.buffered.len() as u32) < state.buffer_size {
    state.buffered.push_back(work);
  } else {
    deferred.push(WppcDeferredAction::LogDropped {
      total_seq_nr,
      buffered_len: state.buffered.len(),
      buffer_size: state.buffer_size,
      message_type: core::any::type_name::<A>(),
    });
  }

  state.current_seq_nr += 1;
  collect_maybe_request_next(state, deferred);
}

fn collect_on_replayed_message<A>(
  state: &mut WorkPullingState<A>,
  sent: MessageSent<A>,
  deferred: &mut Vec<WppcDeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  let work = BufferedWork::Replay(sent);
  if let Some(worker_key) = state.find_worker_with_demand() {
    collect_send_to_worker(state, worker_key, work, deferred);
  } else if (state.buffered.len() as u32) < state.buffer_size {
    state.buffered.push_back(work);
  }

  collect_maybe_request_next(state, deferred);
}

/// Collects a send-to-worker action.
fn collect_send_to_worker<A>(
  state: &mut WorkPullingState<A>,
  worker_key: u64,
  work: BufferedWork<A>,
  deferred: &mut Vec<WppcDeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  if let Some(entry) = state.workers.get_mut(&worker_key) {
    entry.has_demand = false;
    let producer_controller = entry.producer_controller.clone();
    let worker_local_seq_nr = entry.next_seq_nr;
    let confirmation_qualifier = entry.producer_id.clone();
    match work {
      | BufferedWork::Fresh { seq_nr, message } => {
        if let (Some(durable_queue), Some(store_ack_adapter)) =
          (state.durable_queue.clone(), state.store_ack_adapter.clone())
        {
          let sent = MessageSent::new(seq_nr, message.clone(), false, confirmation_qualifier.clone(), 0);
          state.pending_stores.insert(seq_nr, PendingDurableStore {
            message,
            worker_key,
            worker_local_seq_nr,
            confirmation_qualifier,
            replay_confirmation_of: None,
          });
          deferred.push(WppcDeferredAction::TellDurableQueue {
            target:  durable_queue,
            message: DurableProducerQueueCommand::store_message_sent(sent, store_ack_adapter),
            timeout: Some(WppcDurableQueueTimeout::Store { seq_nr, attempt: 1 }),
          });
        } else {
          deferred.push(WppcDeferredAction::TellWorker {
            target: producer_controller,
            message: ProducerControllerCommand::msg(message.clone()),
            worker_key,
            worker_local_seq_nr,
          });
          let sent = MessageSent::new(seq_nr, message, false, confirmation_qualifier, 0);
          entry.in_flight.insert(worker_local_seq_nr, sent);
        }
      },
      | BufferedWork::Replay(sent) => {
        let replay_seq_nr = state.current_seq_nr;
        state.current_seq_nr += 1;
        if let (Some(durable_queue), Some(store_ack_adapter)) =
          (state.durable_queue.clone(), state.store_ack_adapter.clone())
        {
          let replayed = MessageSent::new(
            replay_seq_nr,
            sent.message().clone(),
            sent.ack(),
            confirmation_qualifier.clone(),
            sent.timestamp_millis(),
          );
          state.pending_stores.insert(replay_seq_nr, PendingDurableStore {
            message: sent.message().clone(),
            worker_key,
            worker_local_seq_nr,
            confirmation_qualifier,
            replay_confirmation_of: Some((sent.confirmation_qualifier().to_string(), sent.seq_nr())),
          });
          deferred.push(WppcDeferredAction::TellDurableQueue {
            target:  durable_queue,
            message: DurableProducerQueueCommand::store_message_sent(replayed, store_ack_adapter),
            timeout: Some(WppcDurableQueueTimeout::Store { seq_nr: replay_seq_nr, attempt: 1 }),
          });
        } else {
          deferred.push(WppcDeferredAction::TellWorker {
            target: producer_controller,
            message: ProducerControllerCommand::msg(sent.message().clone()),
            worker_key,
            worker_local_seq_nr,
          });
          let replayed = MessageSent::new(
            replay_seq_nr,
            sent.message().clone(),
            sent.ack(),
            confirmation_qualifier,
            sent.timestamp_millis(),
          );
          entry.in_flight.insert(worker_local_seq_nr, replayed);
        }
      },
    }
  }
}

/// Drains buffered messages to workers with demand.
fn collect_drain_buffered<A>(state: &mut WorkPullingState<A>, deferred: &mut Vec<WppcDeferredAction<A>>)
where
  A: Clone + Send + Sync + 'static, {
  while let Some(work) = state.buffered.pop_front() {
    if let Some(worker_key) = state.find_worker_with_demand() {
      collect_send_to_worker(state, worker_key, work, deferred);
    } else {
      // デマンドのあるワーカーがいない場合、メッセージをバッファに戻して終了する
      state.buffered.push_front(work);
      break;
    }
  }
}

/// Handles an internal demand signal from a per-worker ProducerController.
fn collect_on_internal_demand<A>(
  state: &mut WorkPullingState<A>,
  request: &ProducerControllerRequestNext<A>,
  deferred: &mut Vec<WppcDeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  for entry in state.workers.values_mut() {
    if entry.producer_id == request.producer_id() {
      entry.has_demand = true;
      entry.next_seq_nr = request.current_seq_nr();
      if request.confirmed_seq_nr() > entry.confirmed_seq_nr {
        entry.confirmed_seq_nr = request.confirmed_seq_nr();
        let confirmed_local_seq_nrs: Vec<u64> =
          entry.in_flight.keys().copied().filter(|seq_nr| *seq_nr <= request.confirmed_seq_nr()).collect();
        let mut highest_total_seq_nr: Option<u64> = None;
        for seq_nr in confirmed_local_seq_nrs {
          if let Some(sent) = entry.in_flight.remove(&seq_nr) {
            let total_seq_nr = sent.seq_nr();
            highest_total_seq_nr = Some(highest_total_seq_nr.map_or(total_seq_nr, |current| current.max(total_seq_nr)));
          }
        }
        if let (Some(total_seq_nr), Some(durable_queue)) = (highest_total_seq_nr, state.durable_queue.clone()) {
          deferred.push(WppcDeferredAction::TellDurableQueue {
            target:  durable_queue,
            message: DurableProducerQueueCommand::store_message_confirmed(total_seq_nr, entry.producer_id.clone(), 0),
            timeout: None,
          });
        }
      }
      break;
    }
  }

  collect_drain_buffered(state, deferred);
  collect_maybe_request_next(state, deferred);
}

/// Collects a RequestNext to the producer if there is demand.
fn collect_maybe_request_next<A>(state: &mut WorkPullingState<A>, deferred: &mut Vec<WppcDeferredAction<A>>)
where
  A: Clone + Send + Sync + 'static, {
  if state.awaiting_load {
    return;
  }
  if state.awaiting_msg {
    return;
  }
  if state.producer.is_none() || state.send_adapter.is_none() {
    return;
  }
  if state.buffered.iter().any(|work| matches!(work, BufferedWork::Replay(_))) {
    return;
  }

  let buffer_has_room = (state.buffered.len() as u32) < state.buffer_size;
  let has_demand = state.any_worker_has_demand() || buffer_has_room;
  if !has_demand {
    return;
  }

  if let (Some(producer), Some(send_adapter)) = (&state.producer, &state.send_adapter) {
    let request_next = WorkPullingProducerControllerRequestNext::new(
      state.producer_id.clone(),
      state.current_seq_nr,
      0,
      send_adapter.clone(),
    );
    deferred.push(WppcDeferredAction::RequestNext(producer.clone(), request_next));
    state.awaiting_msg = true;
  }
}

fn collect_on_durable_queue_message_stored<A>(
  state: &mut WorkPullingState<A>,
  ack: &StoreMessageSentAck,
  self_ref: &TypedActorRef<WorkPullingProducerControllerCommand<A>>,
  deferred: &mut Vec<WppcDeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  let Some(pending) = state.pending_stores.remove(&ack.stored_seq_nr()) else {
    return;
  };

  if let Some((old_confirmation_qualifier, old_seq_nr)) = pending.replay_confirmation_of
    && let Some(durable_queue) = state.durable_queue.clone()
  {
    deferred.push(WppcDeferredAction::TellDurableQueue {
      target:  durable_queue,
      message: DurableProducerQueueCommand::store_message_confirmed(old_seq_nr, old_confirmation_qualifier, 0),
      timeout: None,
    });
  }

  if let Some(entry) = state.workers.get_mut(&pending.worker_key) {
    let sent = MessageSent::new(ack.stored_seq_nr(), pending.message.clone(), false, pending.confirmation_qualifier, 0);
    entry.in_flight.insert(pending.worker_local_seq_nr, sent);
    deferred.push(WppcDeferredAction::TellWorker {
      target:              entry.producer_controller.clone(),
      message:             ProducerControllerCommand::msg(pending.message),
      worker_key:          pending.worker_key,
      worker_local_seq_nr: pending.worker_local_seq_nr,
    });
  } else {
    let replay = MessageSent::new(ack.stored_seq_nr(), pending.message, false, pending.confirmation_qualifier, 0);
    deferred.push(WppcDeferredAction::TellSelf(
      self_ref.clone(),
      WorkPullingProducerControllerCommand::replay_stored_message(replay),
    ));
  }
}

/// Executes deferred actions outside the state lock.
fn execute_wppc_deferred<A>(
  actions: Vec<WppcDeferredAction<A>>,
  ctx: &mut TypedActorContext<'_, WorkPullingProducerControllerCommand<A>>,
  state: &ArcShared<RuntimeMutex<WorkPullingState<A>>>,
  internal_ask_timeout: core::time::Duration,
  durable_queue_request_timeout: core::time::Duration,
) where
  A: Clone + Send + Sync + 'static, {
  let mut pending: VecDeque<WppcDeferredAction<A>> = actions.into_iter().collect();
  while let Some(action) = pending.pop_front() {
    match action {
      | WppcDeferredAction::TellWorker { mut target, message, worker_key, worker_local_seq_nr } => {
        if let Err(error) = target.try_tell(message) {
          let message =
            alloc::format!("WorkPullingProducerController failed to send message to worker controller: {:?}", error);
          ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
        } else if let Err(error) = ctx.schedule_once(
          internal_ask_timeout,
          ctx.self_ref(),
          WorkPullingProducerControllerCommand::worker_delivery_timed_out(worker_key, worker_local_seq_nr),
        ) {
          let message =
            alloc::format!("WorkPullingProducerController failed to schedule worker delivery timeout: {:?}", error);
          ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
        }
      },
      | WppcDeferredAction::TellDurableQueue { mut target, message, timeout } => {
        if let Err(error) = target.try_tell(message) {
          let message = alloc::format!("WorkPullingProducerController failed to talk to durable queue: {:?}", error);
          ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
          if let Err(stop_error) = ctx.stop_self() {
            let stop_message = alloc::format!(
              "WorkPullingProducerController failed to stop after durable queue failure: {:?}",
              stop_error
            );
            ctx.system().emit_log(LogLevel::Warn, stop_message, Some(ctx.pid()), None);
          }
        } else if let Some(timeout) = timeout {
          let command = match timeout {
            | WppcDurableQueueTimeout::Load { attempt } => {
              WorkPullingProducerControllerCommand::durable_queue_load_timed_out(attempt)
            },
            | WppcDurableQueueTimeout::Store { seq_nr, attempt } => {
              WorkPullingProducerControllerCommand::durable_queue_store_timed_out(seq_nr, attempt)
            },
          };
          if let Err(error) = ctx.schedule_once(durable_queue_request_timeout, ctx.self_ref(), command) {
            let message =
              alloc::format!("WorkPullingProducerController failed to schedule durable queue timeout: {:?}", error);
            ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
          }
        }
      },
      | WppcDeferredAction::TellSelf(mut target, msg) => {
        if let Err(error) = target.try_tell(msg) {
          let message =
            alloc::format!("WorkPullingProducerController failed to enqueue internal replay command: {:?}", error);
          ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
        }
      },
      | WppcDeferredAction::TellWorkerStats(mut target, msg) => {
        if let Err(error) = target.try_tell(msg) {
          let message = alloc::format!("WorkPullingProducerController failed to send worker stats reply: {:?}", error);
          ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
        }
      },
      | WppcDeferredAction::RequestNext(mut target, msg) => {
        if let Err(error) = target.try_tell(msg) {
          let message = alloc::format!("WorkPullingProducerController failed to request next work item: {:?}", error);
          ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
        }
      },
      | WppcDeferredAction::StopWorkerPc(mut pc_ref) => {
        if let Err(error) = stop_worker_producer_controller(&mut pc_ref) {
          let message = alloc::format!("WorkPullingProducerController failed to stop worker controller: {:?}", error);
          ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
        }
      },
      | WppcDeferredAction::SpawnWorker {
        worker_ref,
        producer_id: pc_producer_id,
        demand_adapter,
        producer_controller_settings,
      } => {
        // ワーカー PC を生成し、先に state に登録してから tell() する。
        // インラインディスパッチで InternalDemand が即座に返るため、
        // 登録前に tell() すると demand シグナルが消失する。
        if let Some((entry, pc_ref)) =
          spawn_worker_actor::<A>(ctx, &worker_ref, &pc_producer_id, &producer_controller_settings)
        {
          // 先にワーカーを登録する
          state.lock().workers.insert(pid_key(&worker_ref), entry);

          // ワーカー PC に Start と RegisterConsumer を送信する。
          // InternalDemand がインラインで処理され、state.workers に
          // 登録済みなので demand シグナルが正しく反映される。
          let mut pc_start = pc_ref.clone();
          if let Err(error) = pc_start.try_tell(ProducerController::start(demand_adapter.clone())) {
            let message =
              alloc::format!("WorkPullingProducerController failed to start worker controller: {:?}", error);
            ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
          }

          let cc_ref = TypedActorRef::<ConsumerControllerCommand<A>>::from_untyped(worker_ref.clone());
          let mut pc_reg = pc_ref;
          if let Err(error) = pc_reg.try_tell(ProducerController::register_consumer(cc_ref)) {
            let message = alloc::format!(
              "WorkPullingProducerController failed to register worker consumer controller: {:?}",
              error
            );
            ctx.system().emit_log(LogLevel::Warn, message, Some(ctx.pid()), None);
          }

          // バッファ済みメッセージを排出する
          let mut s = state.lock();
          let mut drain_deferred = Vec::new();
          collect_drain_buffered(&mut s, &mut drain_deferred);
          collect_maybe_request_next(&mut s, &mut drain_deferred);
          drop(s);
          pending.extend(drain_deferred);
        }
      },
      | WppcDeferredAction::LogDropped { total_seq_nr, buffered_len, buffer_size, message_type } => {
        ctx.system().emit_log(
          LogLevel::Warn,
          alloc::format!(
            "WorkPullingProducerController dropped buffered message: seq_nr={}, buffered_len={}, buffer_size={}, message_type={}",
            total_seq_nr,
            buffered_len,
            buffer_size,
            message_type
          ),
          Some(ctx.pid()),
          None,
        );
      },
    }
  }
}

fn stop_worker_producer_controller<A>(
  pc_ref: &mut TypedActorRef<ProducerControllerCommand<A>>,
) -> Result<(), SendError>
where
  A: Clone + Send + Sync + 'static, {
  pc_ref.as_untyped_mut().try_tell(AnyMessage::new(SystemMessage::PoisonPill)).map(|_| ())
}

/// Result of spawning a per-worker ProducerController.
type SpawnedWorker<A> = (WorkerEntry<A>, TypedActorRef<ProducerControllerCommand<A>>);

/// Spawns a per-worker ProducerController actor and returns the worker entry
/// and PC ref. Does NOT send Start/RegisterConsumer — the caller must do that
/// after inserting the entry into `state.workers` so that inline-dispatched
/// `InternalDemand` signals find the registered worker.
fn spawn_worker_actor<A>(
  ctx: &mut TypedActorContext<'_, WorkPullingProducerControllerCommand<A>>,
  worker_ref: &ActorRef,
  pc_producer_id: &str,
  producer_controller_settings: &ProducerControllerSettings,
) -> Option<SpawnedWorker<A>>
where
  A: Clone + Send + Sync + 'static, {
  let pc_id = pc_producer_id.to_string();
  let producer_controller_settings = producer_controller_settings.clone();

  let pc_props = TypedProps::<ProducerControllerCommand<A>>::from_behavior_factory(move || {
    ProducerController::behavior_with_settings::<A>(pc_id.clone(), &producer_controller_settings, None)
  });

  let pc_ref = match ctx.spawn_child(&pc_props) {
    | Ok(child) => child.into_actor_ref(),
    | Err(error) => {
      let message = alloc::format!("Failed to spawn ProducerController for worker {}: {:?}", worker_ref.pid(), error);
      ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()), None);
      return None;
    },
  };

  let entry = WorkerEntry {
    producer_id:         pc_producer_id.to_string(),
    producer_controller: pc_ref.clone(),
    next_seq_nr:         1,
    confirmed_seq_nr:    0,
    in_flight:           BTreeMap::new(),
    has_demand:          false,
  };

  Some((entry, pc_ref))
}
