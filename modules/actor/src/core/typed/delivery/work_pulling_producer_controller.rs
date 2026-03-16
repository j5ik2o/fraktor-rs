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
  actor::actor_ref::ActorRef,
  error::ActorError,
  event::logging::LogLevel,
  typed::{
    Behaviors, Listing, Receptionist, ServiceKey,
    actor::TypedActorRef,
    behavior::Behavior,
    delivery::{
      ConsumerControllerCommand, ProducerController, ProducerControllerCommand, ProducerControllerRequestNext,
      WorkPullingProducerControllerCommand, WorkPullingProducerControllerRequestNext,
      WorkPullingProducerControllerSettings, WorkerStats,
      work_pulling_producer_controller_command::WorkPullingProducerControllerCommandKind,
    },
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
  Tell(TypedActorRef<ProducerControllerCommand<A>>, ProducerControllerCommand<A>),
  TellWorkerStats(TypedActorRef<WorkerStats>, WorkerStats),
  RequestNext(TypedActorRef<WorkPullingProducerControllerRequestNext<A>>, WorkPullingProducerControllerRequestNext<A>),
  /// Spawn a per-worker ProducerController and wire it up.
  SpawnWorker {
    worker_ref:     ActorRef,
    producer_id:    String,
    demand_adapter: TypedActorRef<ProducerControllerRequestNext<A>>,
  },
}

/// Tracks a single worker and its associated `ProducerController`.
struct WorkerEntry<A>
where
  A: Clone + Send + Sync + 'static, {
  /// The worker's untyped actor ref (for identity tracking).
  worker_ref:          ActorRef,
  /// The per-worker `ProducerController` ref.
  producer_controller: TypedActorRef<ProducerControllerCommand<A>>,
  /// Whether this worker has pending demand.
  has_demand:          bool,
}

/// Internal state for the work-pulling producer controller.
struct WorkPullingState<A>
where
  A: Clone + Send + Sync + 'static, {
  producer_id:    String,
  current_seq_nr: u64,
  producer:       Option<TypedActorRef<WorkPullingProducerControllerRequestNext<A>>>,
  send_adapter:   Option<TypedActorRef<A>>,
  /// Adapter that per-worker PCs use to signal demand back to the WPPC.
  demand_adapter: Option<TypedActorRef<ProducerControllerRequestNext<A>>>,
  /// Workers keyed by `Pid::value()`.
  workers:        BTreeMap<u64, WorkerEntry<A>>,
  /// Round-robin index for distributing messages.
  next_worker:    usize,
  /// Buffered messages when no worker has demand.
  buffered:       VecDeque<A>,
  /// Maximum buffer size.
  buffer_size:    u32,
  /// Whether we have sent a RequestNext to the producer and are awaiting a Msg.
  awaiting_msg:   bool,
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
/// [`ServiceKey`](crate::core::typed::ServiceKey) and the
/// [`Receptionist`](crate::core::typed::Receptionist).
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

  /// Creates the work-pulling producer controller behavior with default
  /// settings.
  #[must_use]
  pub fn behavior<A>(
    producer_id: impl Into<String>,
    worker_service_key: ServiceKey<ConsumerControllerCommand<A>>,
  ) -> Behavior<WorkPullingProducerControllerCommand<A>>
  where
    A: Clone + Send + Sync + 'static, {
    Self::behavior_with_settings(producer_id, worker_service_key, &WorkPullingProducerControllerSettings::new())
  }

  /// Creates the work-pulling producer controller behavior with custom
  /// settings.
  #[must_use]
  pub(crate) fn behavior_with_settings<A>(
    producer_id: impl Into<String>,
    worker_service_key: ServiceKey<ConsumerControllerCommand<A>>,
    settings: &WorkPullingProducerControllerSettings,
  ) -> Behavior<WorkPullingProducerControllerCommand<A>>
  where
    A: Clone + Send + Sync + 'static, {
    let producer_id = producer_id.into();
    let buffer_size = settings.buffer_size();

    Behaviors::setup(move |ctx| {
      let _self_ref = ctx.self_ref();

      // メッセージアダプタを作成: A → WorkPullingProducerControllerCommand::Msg
      let send_adapter = match ctx.message_adapter(|a: A| Ok(WorkPullingProducerControllerCommand::msg(a))) {
        | Ok(adapter) => adapter,
        | Err(error) => {
          let message = alloc::format!("WorkPullingProducerController failed to create send adapter: {:?}", error);
          ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()));
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
          ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()));
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
          ctx.system().emit_log(LogLevel::Error, message, Some(ctx.pid()));
          return Behaviors::stopped();
        },
      };

      // ワーカー検出のために Receptionist をサブスクライブする。
      subscribe_to_receptionist(ctx, &worker_service_key, &listing_adapter);

      let state = ArcShared::new(RuntimeMutex::new(WorkPullingState::<A>::new(producer_id.clone(), buffer_size)));
      {
        let mut s = state.lock();
        s.send_adapter = Some(send_adapter);
        s.demand_adapter = Some(demand_adapter);
      }

      let producer_id_inner = producer_id.clone();
      Behaviors::receive_message(move |ctx, command: &WorkPullingProducerControllerCommand<A>| {
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
              collect_on_worker_listing(&mut state, listing, &producer_id_inner, &mut deferred);
            },
            | WorkPullingProducerControllerCommandKind::InternalDemand { request } => {
              collect_on_internal_demand(&mut state, request, &mut deferred);
            },
          }
          deferred
        }; // ステートロックはここで解放される

        execute_wppc_deferred(deferred, ctx, &state)?;
        Ok(Behaviors::same())
      })
    })
  }
}

/// Subscribes to the Receptionist for a specific worker service key.
fn subscribe_to_receptionist<A>(
  ctx: &mut crate::core::typed::actor::TypedActorContext<'_, WorkPullingProducerControllerCommand<A>>,
  worker_service_key: &ServiceKey<ConsumerControllerCommand<A>>,
  listing_adapter: &TypedActorRef<Listing>,
) where
  A: Clone + Send + Sync + 'static, {
  let subscribe_cmd = Receptionist::subscribe(worker_service_key, listing_adapter.clone());
  let system = ctx.system();
  if let Some(mut receptionist_ref) = system.receptionist_ref() {
    let _ = receptionist_ref.tell(subscribe_cmd);
  }
}

/// Collects deferred actions for a worker listing update.
fn collect_on_worker_listing<A>(
  state: &mut WorkPullingState<A>,
  listing: &Listing,
  producer_id: &str,
  deferred: &mut Vec<WppcDeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  let current_keys: BTreeSet<u64> = listing.refs().iter().map(pid_key).collect();

  // リスティングに存在しなくなったワーカーを削除する。
  let removed_keys: Vec<u64> = state.workers.keys().filter(|k| !current_keys.contains(k)).copied().collect();

  for key in &removed_keys {
    state.workers.remove(key);
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
        worker_ref:     actor_ref.clone(),
        producer_id:    pc_producer_id,
        demand_adapter: demand_adapter.clone(),
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
  if let Some(worker_key) = state.find_worker_with_demand() {
    collect_send_to_worker(state, worker_key, message, deferred);
  } else if (state.buffered.len() as u32) < state.buffer_size {
    state.buffered.push_back(message);
  }
  // バッファが満杯かつデマンドのあるワーカーがない場合、メッセージは暗黙的に破棄される。

  state.current_seq_nr += 1;
  collect_maybe_request_next(state, deferred);
}

/// Collects a send-to-worker action.
fn collect_send_to_worker<A>(
  state: &mut WorkPullingState<A>,
  worker_key: u64,
  message: A,
  deferred: &mut Vec<WppcDeferredAction<A>>,
) where
  A: Clone + Send + Sync + 'static, {
  if let Some(entry) = state.workers.get_mut(&worker_key) {
    deferred.push(WppcDeferredAction::Tell(entry.producer_controller.clone(), ProducerControllerCommand::msg(message)));
    entry.has_demand = false;
  }
}

/// Drains buffered messages to workers with demand.
fn collect_drain_buffered<A>(state: &mut WorkPullingState<A>, deferred: &mut Vec<WppcDeferredAction<A>>)
where
  A: Clone + Send + Sync + 'static, {
  while !state.buffered.is_empty() {
    if let Some(worker_key) = state.find_worker_with_demand() {
      let message = state.buffered.pop_front().unwrap();
      collect_send_to_worker(state, worker_key, message, deferred);
    } else {
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
  let pid_str = request.producer_id();
  for entry in state.workers.values_mut() {
    let worker_pid = entry.worker_ref.pid();
    let expected_suffix = alloc::format!("-worker-{}", worker_pid);
    if pid_str.ends_with(&expected_suffix) {
      entry.has_demand = true;
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
  if state.awaiting_msg {
    return;
  }
  if state.producer.is_none() || state.send_adapter.is_none() {
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

/// Executes deferred actions outside the state lock.
fn execute_wppc_deferred<A>(
  actions: Vec<WppcDeferredAction<A>>,
  ctx: &mut crate::core::typed::actor::TypedActorContext<'_, WorkPullingProducerControllerCommand<A>>,
  state: &ArcShared<RuntimeMutex<WorkPullingState<A>>>,
) -> Result<(), ActorError>
where
  A: Clone + Send + Sync + 'static, {
  for action in actions {
    match action {
      | WppcDeferredAction::Tell(mut target, msg) => {
        target.tell(msg).map_err(|e| ActorError::from_send_error(&e))?;
      },
      | WppcDeferredAction::TellWorkerStats(mut target, msg) => {
        target.tell(msg).map_err(|e| ActorError::from_send_error(&e))?;
      },
      | WppcDeferredAction::RequestNext(mut target, msg) => {
        target.tell(msg).map_err(|e| ActorError::from_send_error(&e))?;
      },
      | WppcDeferredAction::SpawnWorker { worker_ref, producer_id: pc_producer_id, demand_adapter } => {
        if let Some(entry) = spawn_and_wire_worker::<A>(ctx, &worker_ref, &pc_producer_id, &demand_adapter)? {
          // 新しいワーカーエントリを登録するためロックを短時間再取得する。
          state.lock().workers.insert(pid_key(&worker_ref), entry);
        }
      },
    }
  }
  Ok(())
}

/// Spawns a per-worker ProducerController, wires it up, and returns the
/// worker entry to be inserted into state.
fn spawn_and_wire_worker<A>(
  ctx: &mut crate::core::typed::actor::TypedActorContext<'_, WorkPullingProducerControllerCommand<A>>,
  worker_ref: &ActorRef,
  pc_producer_id: &str,
  demand_adapter: &TypedActorRef<ProducerControllerRequestNext<A>>,
) -> Result<Option<WorkerEntry<A>>, ActorError>
where
  A: Clone + Send + Sync + 'static, {
  let system = ctx.system();
  let pc_id = pc_producer_id.to_string();

  let pc_props = crate::core::typed::TypedProps::<ProducerControllerCommand<A>>::from_behavior_factory(move || {
    ProducerController::behavior::<A>(pc_id.clone())
  });

  let pc_cell = match system.as_untyped().spawn(pc_props.to_untyped()) {
    | Ok(cell) => cell,
    | Err(error) => {
      let message = alloc::format!("Failed to spawn ProducerController for worker {}: {:?}", worker_ref.pid(), error);
      system.emit_log(LogLevel::Error, message, None);
      return Ok(None);
    },
  };

  let pc_ref = TypedActorRef::<ProducerControllerCommand<A>>::from_untyped(pc_cell.actor_ref().clone());

  // ワーカー単位の ProducerController を開始する。
  let mut pc_ref_start = pc_ref.clone();
  pc_ref_start.tell(ProducerController::start(demand_adapter.clone())).map_err(|e| ActorError::from_send_error(&e))?;

  // ワーカーの ConsumerController を登録する。
  let cc_ref = TypedActorRef::<ConsumerControllerCommand<A>>::from_untyped(worker_ref.clone());
  let mut pc_ref_clone = pc_ref.clone();
  pc_ref_clone.tell(ProducerController::register_consumer(cc_ref)).map_err(|e| ActorError::from_send_error(&e))?;

  Ok(Some(WorkerEntry {
    worker_ref:          worker_ref.clone(),
    producer_controller: pc_ref,
    has_demand:          true,
  }))
}
