//! Phase 14.5 contention observation test for `BalancingDispatcher`.
//!
//! Runs a multi-threaded `BalancingDispatcher` workload (4 team members,
//! 1000 envelopes) using a Tokio multi-thread executor and prints the
//! observed per-actor distribution to stderr. The numbers are intentionally
//! not asserted: the V1 trade-off documented in
//! `openspec/changes/dispatcher-pekko-1n-redesign/design.md §9` (no active
//! wake of idle team members → potential skew while a busy receiver
//! drains) is expected to manifest as uneven distribution, but exact ratios
//! depend on OS scheduler timing and would make the test flaky.
//!
//! The companion bench harness lives at
//! `modules/actor-adaptor-std/benches/balancing_dispatcher.rs` (compile
//! checked but currently blocked by an unrelated `critical-section` impl
//! linker issue in the bench profile that affects every bench in the
//! workspace).

use std::{
  boxed::Box,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc::{Receiver, SyncSender, sync_channel},
  },
  time::{Duration, Instant},
  vec::Vec,
};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorCellState, ActorCellStateShared, ActorCellStateSharedFactory, ActorContext, ActorShared,
    ActorSharedFactory, ReceiveTimeoutState, ReceiveTimeoutStateShared, ReceiveTimeoutStateSharedFactory,
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, ActorRefSenderSharedFactory},
    error::ActorError,
    messaging::{
      AnyMessage, AnyMessageView,
      message_invoker::{MessageInvoker, MessageInvokerShared, MessageInvokerSharedFactory},
    },
    props::Props,
    setup::ActorSystemConfig,
  },
  dispatch::dispatcher::{
    BalancingDispatcherConfigurator, DEFAULT_DISPATCHER_ID, DefaultDispatcherConfigurator, DispatcherSettings,
    ExecuteError, Executor, ExecutorFactory, ExecutorShared, ExecutorSharedFactory, MessageDispatcher,
    MessageDispatcherConfigurator, MessageDispatcherShared, MessageDispatcherSharedFactory,
    PinnedDispatcherConfigurator, SharedMessageQueue, SharedMessageQueueFactory, TrampolineState,
  },
  event::stream::{
    EventStream, EventStreamShared, EventStreamSharedFactory, EventStreamSubscriber, EventStreamSubscriberShared,
    EventStreamSubscriberSharedFactory,
  },
  system::{
    ActorSystem,
    shared_factory::{BuiltinSpinSharedFactory, MailboxSharedSet, MailboxSharedSetFactory},
  },
};
use fraktor_utils_core_rs::core::sync::ArcShared;
use tokio::runtime::Handle;

use crate::std::{
  default_tick_driver_config,
  dispatch::dispatcher::{PinnedExecutorFactory, TokioExecutor, TokioExecutorFactory},
};

const BALANCING_DISPATCHER_ID: &str = "balancing-contention";
const SPAWN_TIMEOUT: Duration = Duration::from_secs(5);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

struct NoopExecutor;

impl Executor for NoopExecutor {
  fn execute(&mut self, _task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
    Ok(())
  }

  fn shutdown(&mut self) {}
}

struct CountingLockProvider {
  inner: BuiltinSpinSharedFactory,
  executor_shared_calls: Arc<AtomicUsize>,
  dispatcher_shared_calls: Arc<AtomicUsize>,
  shared_message_queue_calls: Arc<AtomicUsize>,
}

impl CountingLockProvider {
  fn new() -> (Arc<AtomicUsize>, Arc<AtomicUsize>, Arc<AtomicUsize>, Self) {
    let executor_shared_calls = Arc::new(AtomicUsize::new(0));
    let dispatcher_shared_calls = Arc::new(AtomicUsize::new(0));
    let shared_message_queue_calls = Arc::new(AtomicUsize::new(0));
    let provider = Self {
      inner: BuiltinSpinSharedFactory::new(),
      executor_shared_calls: Arc::clone(&executor_shared_calls),
      dispatcher_shared_calls: Arc::clone(&dispatcher_shared_calls),
      shared_message_queue_calls: Arc::clone(&shared_message_queue_calls),
    };
    (executor_shared_calls, dispatcher_shared_calls, shared_message_queue_calls, provider)
  }
}

impl MessageDispatcherSharedFactory for CountingLockProvider {
  fn create_message_dispatcher_shared(&self, dispatcher: Box<dyn MessageDispatcher>) -> MessageDispatcherShared {
    self.dispatcher_shared_calls.fetch_add(1, Ordering::SeqCst);
    MessageDispatcherSharedFactory::create_message_dispatcher_shared(&self.inner, dispatcher)
  }
}

impl ExecutorSharedFactory for CountingLockProvider {
  fn create_executor_shared(&self, executor: Box<dyn Executor>, trampoline: TrampolineState) -> ExecutorShared {
    self.executor_shared_calls.fetch_add(1, Ordering::SeqCst);
    self.inner.create_executor_shared(executor, trampoline)
  }
}

impl ActorRefSenderSharedFactory for CountingLockProvider {
  fn create_actor_ref_sender_shared(&self, sender: Box<dyn ActorRefSender>) -> ActorRefSenderShared {
    ActorRefSenderSharedFactory::create_actor_ref_sender_shared(&self.inner, sender)
  }
}

impl ActorSharedFactory for CountingLockProvider {
  fn create(&self, actor: Box<dyn Actor + Send>) -> ActorShared {
    ActorSharedFactory::create(&self.inner, actor)
  }
}

impl ActorCellStateSharedFactory for CountingLockProvider {
  fn create_actor_cell_state_shared(&self, state: ActorCellState) -> ActorCellStateShared {
    ActorCellStateSharedFactory::create_actor_cell_state_shared(&self.inner, state)
  }
}

impl ReceiveTimeoutStateSharedFactory for CountingLockProvider {
  fn create_receive_timeout_state_shared(&self, state: Option<ReceiveTimeoutState>) -> ReceiveTimeoutStateShared {
    ReceiveTimeoutStateSharedFactory::create_receive_timeout_state_shared(&self.inner, state)
  }
}

impl MessageInvokerSharedFactory for CountingLockProvider {
  fn create(&self, invoker: Box<dyn MessageInvoker>) -> MessageInvokerShared {
    MessageInvokerSharedFactory::create(&self.inner, invoker)
  }
}

impl SharedMessageQueueFactory for CountingLockProvider {
  fn create(&self) -> SharedMessageQueue {
    self.shared_message_queue_calls.fetch_add(1, Ordering::SeqCst);
    SharedMessageQueueFactory::create(&self.inner)
  }
}

impl EventStreamSharedFactory for CountingLockProvider {
  fn create(&self, stream: EventStream) -> EventStreamShared {
    EventStreamSharedFactory::create(&self.inner, stream)
  }
}

impl EventStreamSubscriberSharedFactory for CountingLockProvider {
  fn create(&self, subscriber: Box<dyn EventStreamSubscriber>) -> EventStreamSubscriberShared {
    EventStreamSubscriberSharedFactory::create(&self.inner, subscriber)
  }
}

impl MailboxSharedSetFactory for CountingLockProvider {
  fn create(&self) -> MailboxSharedSet {
    MailboxSharedSetFactory::create(&self.inner)
  }
}

struct CountingActor {
  per_actor: Arc<AtomicUsize>,
  total:     Arc<AtomicUsize>,
}

impl Actor for CountingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    self.per_actor.fetch_add(1, Ordering::Relaxed);
    self.total.fetch_add(1, Ordering::Release);
    Ok(())
  }
}

struct SpawnTeamMember {
  reply_to:      SyncSender<ActorRef>,
  per_actor:     Arc<AtomicUsize>,
  total:         Arc<AtomicUsize>,
  dispatcher_id: String,
}

struct TeamGuardian;

impl Actor for TeamGuardian {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(command) = message.downcast_ref::<SpawnTeamMember>() {
      let per = command.per_actor.clone();
      let total = command.total.clone();
      let did = command.dispatcher_id.clone();
      let props = Props::from_fn(move || CountingActor { per_actor: per.clone(), total: total.clone() })
        .with_dispatcher_id(did.clone());
      let child = ctx.spawn_child(&props).map_err(|_| ActorError::recoverable("spawn team member failed"))?;
      command.reply_to.send(child.into_actor_ref()).expect("team member ref");
    }
    Ok(())
  }
}

fn build_system() -> ActorSystem {
  let handle = Handle::current();

  // Use the default throughput (5). With the lost-wake-up fix in
  // `Mailbox::run` (Phase 14.5 follow-up), the dispatcher correctly
  // re-schedules itself across throughput boundaries, so a 1000-envelope
  // batch drains correctly even though each `mailbox.run` invocation only
  // processes a small slice.
  let config = ActorSystemConfig::default().with_tick_driver(default_tick_driver_config());
  let message_dispatcher_shared_factory = config.message_dispatcher_shared_factory().clone();
  let shared_message_queue_factory = config.shared_message_queue_factory().clone();
  let mailbox_shared_set_factory = config.mailbox_shared_set_factory().clone();
  let default_settings = DispatcherSettings::with_defaults(DEFAULT_DISPATCHER_ID);
  let default_executor = BuiltinSpinSharedFactory::new()
    .create_executor_shared(Box::new(TokioExecutor::new(handle.clone())), TrampolineState::new());
  let default_configurator: Box<dyn MessageDispatcherConfigurator> = Box::new(DefaultDispatcherConfigurator::new(
    &default_settings,
    default_executor,
    &message_dispatcher_shared_factory,
  ));

  let balancing_settings = DispatcherSettings::with_defaults(BALANCING_DISPATCHER_ID);
  let balancing_executor = BuiltinSpinSharedFactory::new()
    .create_executor_shared(Box::new(TokioExecutor::new(handle)), TrampolineState::new());
  let balancing_configurator: Box<dyn MessageDispatcherConfigurator> = Box::new(BalancingDispatcherConfigurator::new(
    &balancing_settings,
    balancing_executor,
    &message_dispatcher_shared_factory,
    &shared_message_queue_factory,
    &mailbox_shared_set_factory,
  ));

  let config = config
    .with_dispatcher_configurator(DEFAULT_DISPATCHER_ID, ArcShared::new(default_configurator))
    .with_dispatcher_configurator(BALANCING_DISPATCHER_ID, ArcShared::new(balancing_configurator));
  let props = Props::from_fn(|| TeamGuardian);
  ActorSystem::new_with_config(&props, &config).expect("actor system")
}

fn spawn_team(
  system: &ActorSystem,
  team_size: usize,
  dispatcher_id: &str,
) -> (Vec<ActorRef>, Vec<Arc<AtomicUsize>>, Arc<AtomicUsize>) {
  let total = Arc::new(AtomicUsize::new(0));
  let per_actor: Vec<Arc<AtomicUsize>> = (0..team_size).map(|_| Arc::new(AtomicUsize::new(0))).collect();
  let mut refs = Vec::with_capacity(team_size);
  for index in 0..team_size {
    let (reply_tx, reply_rx): (SyncSender<ActorRef>, Receiver<ActorRef>) = sync_channel(1);
    let command = SpawnTeamMember {
      reply_to:      reply_tx,
      per_actor:     per_actor[index].clone(),
      total:         total.clone(),
      dispatcher_id: dispatcher_id.to_string(),
    };
    system.user_guardian_ref().tell(AnyMessage::new(command));
    let child = reply_rx.recv_timeout(SPAWN_TIMEOUT).expect("team member spawn ack");
    refs.push(child);
  }
  (refs, per_actor, total)
}

fn flood_and_wait(receiver: &ActorRef, total: &Arc<AtomicUsize>, count: usize) -> Duration {
  let started = Instant::now();
  for sequence in 0..count {
    let mut target = receiver.clone();
    target.tell(AnyMessage::new(sequence as u32));
  }
  let after_send = started.elapsed();
  let deadline = started + DRAIN_TIMEOUT;
  let mut last_observed = 0_usize;
  let mut last_progress = Instant::now();
  while total.load(Ordering::Acquire) < count {
    let now = Instant::now();
    let observed = total.load(Ordering::Acquire);
    if observed > last_observed {
      last_observed = observed;
      last_progress = now;
    }
    if now > deadline {
      panic!(
        "timed out waiting for drain: total={} expected={} after_send_ms={} \
         stalled_ms={}",
        observed,
        count,
        after_send.as_millis(),
        last_progress.elapsed().as_millis(),
      );
    }
    std::thread::yield_now();
  }
  started.elapsed()
}

fn snapshot(per_actor: &[Arc<AtomicUsize>]) -> Vec<usize> {
  per_actor.iter().map(|c| c.load(Ordering::Acquire)).collect()
}

#[tokio::test(flavor = "current_thread")]
async fn tokio_executor_factory_new_with_provider_materializes_executor_shared_via_provider() {
  let (executor_shared_calls, dispatcher_shared_calls, _, provider) = CountingLockProvider::new();
  let provider = ArcShared::new(provider);
  let executor_shared_factory: ArcShared<dyn ExecutorSharedFactory> = provider;
  let factory = TokioExecutorFactory::new(Handle::current(), &executor_shared_factory);

  let _executor = factory.create(DEFAULT_DISPATCHER_ID);

  assert_eq!(
    executor_shared_calls.load(Ordering::SeqCst),
    1,
    "tokio executor factory should route executor shared construction through the configured provider"
  );
  assert_eq!(
    dispatcher_shared_calls.load(Ordering::SeqCst),
    0,
    "tokio executor factory should only materialize executor shared handles"
  );
}

#[test]
fn pinned_dispatcher_configurator_uses_provider_aware_executor_factory_for_each_dispatcher_instance() {
  let (executor_shared_calls, dispatcher_shared_calls, _, provider) = CountingLockProvider::new();
  let provider = ArcShared::new(provider);
  let settings = DispatcherSettings::with_defaults("pinned-provider-test");
  let executor_shared_factory: ArcShared<dyn ExecutorSharedFactory> = provider.clone();
  let message_dispatcher_shared_factory: ArcShared<dyn MessageDispatcherSharedFactory> = provider.clone();
  let executor_factory: ArcShared<Box<dyn ExecutorFactory>> =
    ArcShared::new(Box::new(PinnedExecutorFactory::new("provider-aware", &executor_shared_factory)));
  let configurator =
    PinnedDispatcherConfigurator::new(settings, executor_factory, &message_dispatcher_shared_factory, "provider-aware");

  let _first = configurator.dispatcher();
  let _second = configurator.dispatcher();

  assert_eq!(
    executor_shared_calls.load(Ordering::SeqCst),
    2,
    "pinned dispatcher should obtain a provider-built executor for each fresh dispatcher instance"
  );
  assert_eq!(
    dispatcher_shared_calls.load(Ordering::SeqCst),
    2,
    "pinned dispatcher should keep using the provider for dispatcher shared wrapping"
  );
}

#[test]
fn balancing_dispatcher_configurator_materializes_shared_queue_via_provider() {
  let (_, dispatcher_shared_calls, shared_message_queue_calls, provider) = CountingLockProvider::new();
  let provider = ArcShared::new(provider);
  let settings = DispatcherSettings::with_defaults("balancing-provider-test");
  let executor = BuiltinSpinSharedFactory::new().create_executor_shared(Box::new(NoopExecutor), TrampolineState::new());
  let message_dispatcher_shared_factory: ArcShared<dyn MessageDispatcherSharedFactory> = provider.clone();
  let shared_message_queue_factory: ArcShared<dyn SharedMessageQueueFactory> = provider.clone();
  let mailbox_shared_set_factory: ArcShared<dyn MailboxSharedSetFactory> = provider;
  let configurator = BalancingDispatcherConfigurator::new(
    &settings,
    executor,
    &message_dispatcher_shared_factory,
    &shared_message_queue_factory,
    &mailbox_shared_set_factory,
  );

  let _dispatcher = configurator.dispatcher();

  assert_eq!(
    dispatcher_shared_calls.load(Ordering::SeqCst),
    1,
    "balancing dispatcher configurator should still materialize the dispatcher wrapper via the provider"
  );
  assert_eq!(
    shared_message_queue_calls.load(Ordering::SeqCst),
    1,
    "balancing dispatcher should materialize its shared queue via the configured lock provider"
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn balancing_dispatcher_contention_distribution_observation() {
  // Phase 14.5: dynamic observation of the V1 BalancingDispatcher
  // contention behaviour against the trade-off documented in design.md §9.
  // We do NOT assert the exact distribution shape (skew vs even split
  // depends on OS-scheduler timing) but we do dump the per-actor
  // distribution to stderr so the observed numbers become part of the
  // test output record.
  //
  // The bench/test runs at the default `throughput=5` (5 envelopes per
  // mailbox.run drain pass), which is the production-realistic setting and
  // forces the dispatcher to repeatedly re-schedule the mailbox across
  // throughput boundaries. This relies on the lost-wake-up fix in
  // `Mailbox::run`: `run()` now returns the pending-reschedule signal and
  // `MessageDispatcherShared::register_for_execution` re-arms the schedule
  // when work arrived during the drain. Without that fix, this scenario
  // hangs around ~450 envelopes because the late-arriving tells see the
  // mailbox `running`, set `need_reschedule`, and the previous code
  // dropped that signal silently.
  //
  // Invariants asserted:
  // - Every envelope is processed exactly once (sum == batch).
  // - All four team members observe work (`min > 0` → no idle members).
  //
  // Findings:
  // - design.md §9 predicted that V1 (no active wake of idle team members) could leave team members
  //   idle while the receiver drains. In practice the natural drain pattern keeps all four members
  //   working: even when one team member dominates the distribution, every member processes a
  //   non-trivial share of the batch.
  // - The receiver (actor[0]) tends to skew slightly higher because the tells land on its own mailbox
  //   before any sibling can pick them up.
  const TEAM_SIZE: usize = 4;
  const BATCH: usize = 1000;

  let system = tokio::task::spawn_blocking(build_system).await.expect("build system");
  let (refs, per_actor, total) = tokio::task::spawn_blocking({
    let system = system.clone();
    move || spawn_team(&system, TEAM_SIZE, BALANCING_DISPATCHER_ID)
  })
  .await
  .expect("spawn team");

  let elapsed = tokio::task::spawn_blocking({
    let receiver = refs[0].clone();
    let total = total.clone();
    move || flood_and_wait(&receiver, &total, BATCH)
  })
  .await
  .expect("flood");

  let dist = snapshot(&per_actor);
  let total_observed = total.load(Ordering::Acquire);
  let max = dist.iter().copied().max().unwrap_or(0);
  let min = dist.iter().copied().min().unwrap_or(0);
  let idle = dist.iter().filter(|c| **c == 0).count();
  eprintln!(
    "[balancing_dispatcher_contention_distribution_observation] team_size={TEAM_SIZE} batch={BATCH} \
     elapsed_ms={} total={total_observed} per_actor={dist:?} max={max} min={min} idle_team_members={idle}",
    elapsed.as_millis(),
  );

  let sum: usize = dist.iter().sum();
  assert_eq!(sum, BATCH, "every envelope must be processed exactly once: dist={dist:?}");
  let working_actors = dist.iter().filter(|c| **c > 0).count();
  assert_eq!(
    working_actors, TEAM_SIZE,
    "all team members must observe work: dist={dist:?}. \
     If this assertion fails, the V1 contention model has regressed and the \
     receiver is starving its siblings — investigate before relaxing the assertion.",
  );

  tokio::task::spawn_blocking(move || {
    system.terminate().expect("terminate system");
  })
  .await
  .expect("terminate");
}
