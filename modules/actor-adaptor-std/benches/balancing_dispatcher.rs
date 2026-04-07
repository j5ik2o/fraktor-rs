//! Phase 14.5 contention bench for `BalancingDispatcher` (1:N model).
//!
//! Observes how a `BalancingDispatcher` distributes a flood of envelopes
//! across N team members sharing a single message queue, against a
//! `DefaultDispatcher` (1:1) baseline. The bench captures wall-clock
//! throughput per (team_size, batch_size) cell and the per-actor work
//! distribution that the test prints once at startup so the V1 trade-off
//! documented in `dispatcher-pekko-1n-redesign/design.md §9` (no active
//! wake of idle team members → potential skew while a busy receiver
//! drains) is observable from the bench output.

use std::{
  boxed::Box,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc::{Receiver, SyncSender, sync_channel},
  },
  time::Duration,
  vec::Vec,
};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use fraktor_actor_adaptor_rs::std::{default_tick_driver_config, dispatch::dispatcher::TokioExecutor};
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Actor, ActorContext,
    actor_ref::ActorRef,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    setup::ActorSystemConfig,
  },
  dispatch::dispatcher::{
    BalancingDispatcherConfigurator, DEFAULT_DISPATCHER_ID, DefaultDispatcherConfigurator, DispatcherSettings,
    ExecutorShared, MessageDispatcherConfigurator,
  },
  system::ActorSystem,
};
use fraktor_utils_rs::core::sync::ArcShared;
use tokio::runtime::{Builder, Runtime};

const BALANCING_DISPATCHER_ID: &str = "balancing-bench";
const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

/// Counters shared between the bench harness and the actor receivers.
struct TeamCounters {
  per_actor: Vec<Arc<AtomicUsize>>,
  total:     Arc<AtomicUsize>,
}

impl TeamCounters {
  fn new(team_size: usize) -> Self {
    Self {
      per_actor: (0..team_size).map(|_| Arc::new(AtomicUsize::new(0))).collect(),
      total:     Arc::new(AtomicUsize::new(0)),
    }
  }

  fn reset(&self) {
    for c in &self.per_actor {
      c.store(0, Ordering::SeqCst);
    }
    self.total.store(0, Ordering::SeqCst);
  }

  fn snapshot(&self) -> Vec<usize> {
    self.per_actor.iter().map(|c| c.load(Ordering::SeqCst)).collect()
  }
}

/// Receiver actor that bumps both its private counter and the team-wide
/// total. The bench harness spin-checks `total` to detect drain completion.
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

/// Spawn-time message: ask the guardian to materialise one team member with
/// pre-built counter handles and a specific dispatcher id.
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

/// Tokio-backed system with both `default` (DefaultDispatcher) and
/// `balancing-bench` (BalancingDispatcher) configurators wired up.
struct DispatcherBenchSystem {
  runtime: Runtime,
  system:  ActorSystem,
}

impl DispatcherBenchSystem {
  fn new() -> Self {
    let runtime = Builder::new_multi_thread().worker_threads(2).enable_time().build().expect("tokio runtime");
    let handle = runtime.handle().clone();
    let system = runtime.block_on(async {
      let default_settings = DispatcherSettings::with_defaults(DEFAULT_DISPATCHER_ID);
      let default_executor = ExecutorShared::new(TokioExecutor::new(handle.clone()));
      let default_configurator: Box<dyn MessageDispatcherConfigurator> =
        Box::new(DefaultDispatcherConfigurator::new(&default_settings, default_executor));

      let balancing_settings = DispatcherSettings::with_defaults(BALANCING_DISPATCHER_ID);
      let balancing_executor = ExecutorShared::new(TokioExecutor::new(handle));
      let balancing_configurator: Box<dyn MessageDispatcherConfigurator> =
        Box::new(BalancingDispatcherConfigurator::new(&balancing_settings, balancing_executor));

      let config = ActorSystemConfig::default()
        .with_tick_driver(default_tick_driver_config())
        .with_dispatcher_configurator(DEFAULT_DISPATCHER_ID, ArcShared::new(default_configurator))
        .with_dispatcher_configurator(BALANCING_DISPATCHER_ID, ArcShared::new(balancing_configurator));
      let props = Props::from_fn(|| TeamGuardian);
      ActorSystem::new_with_config(&props, &config).expect("actor system")
    });
    Self { runtime, system }
  }

  fn terminate(self) {
    self.runtime.block_on(async {
      self.system.terminate().expect("terminate system");
      self.system.when_terminated().await;
    });
  }
}

/// Spawns `team_size` `CountingActor` instances under the supplied
/// dispatcher id and returns the references plus the shared counters.
fn spawn_team(system: &ActorSystem, team_size: usize, dispatcher_id: &str) -> (Vec<ActorRef>, TeamCounters) {
  let counters = TeamCounters::new(team_size);
  let mut refs = Vec::with_capacity(team_size);
  for index in 0..team_size {
    let (reply_tx, reply_rx): (SyncSender<ActorRef>, Receiver<ActorRef>) = sync_channel(1);
    let command = SpawnTeamMember {
      reply_to:      reply_tx,
      per_actor:     counters.per_actor[index].clone(),
      total:         counters.total.clone(),
      dispatcher_id: dispatcher_id.to_string(),
    };
    system.user_guardian_ref().tell(AnyMessage::new(command));
    let child = reply_rx.recv_timeout(WAIT_TIMEOUT).expect("team member spawn ack");
    refs.push(child);
  }
  (refs, counters)
}

/// Floods the receiver actor with `count` messages and spin-waits until the
/// team-wide counter reaches `count`. The function deliberately blocks the
/// bench harness thread because the wall-clock cost of the drain *is* the
/// metric being measured.
fn flood_and_wait(receiver: &ActorRef, counters: &TeamCounters, count: usize) {
  counters.reset();
  for sequence in 0..count {
    let mut target = receiver.clone();
    target.tell(AnyMessage::new(sequence as u32));
  }
  while counters.total.load(Ordering::Acquire) < count {
    std::thread::yield_now();
  }
}

/// One-time skew snapshot reporter so the bench output documents the
/// per-actor work distribution observed under one representative workload.
/// The numbers are intentionally not asserted: V1 explicitly does not
/// implement teamWork active wake (see design.md §9), so skew is expected
/// and the bench is purely diagnostic.
fn print_distribution_snapshot(team_size: usize, batch: usize) {
  let bench_system = DispatcherBenchSystem::new();
  let (refs, counters) = spawn_team(&bench_system.system, team_size, BALANCING_DISPATCHER_ID);
  flood_and_wait(&refs[0], &counters, batch);
  let snapshot = counters.snapshot();
  let total: usize = snapshot.iter().sum();
  let max = snapshot.iter().copied().max().unwrap_or(0);
  let min = snapshot.iter().copied().min().unwrap_or(0);
  let idle = snapshot.iter().filter(|c| **c == 0).count();
  eprintln!(
    "[balancing_dispatcher::diagnostic] team_size={team_size} batch={batch} total={total} per_actor={snapshot:?} max={max} min={min} idle_team_members={idle}",
  );
  bench_system.terminate();
}

fn bench_balancing_dispatcher(c: &mut Criterion) {
  // Print one diagnostic snapshot before the timed phase. This shows up in
  // the bench output and is the durable record of the V1 trade-off (skew /
  // idle team members) without locking it down as a hard assertion.
  print_distribution_snapshot(4, 1000);

  let mut group = c.benchmark_group("balancing_dispatcher");
  group.sample_size(10);
  group.measurement_time(Duration::from_secs(2));

  for team_size in [2_usize, 4_usize] {
    for batch in [100_usize, 1_000_usize] {
      group.throughput(Throughput::Elements(batch as u64));
      group.bench_with_input(format!("team_{team_size}_batch_{batch}"), &(team_size, batch), |b, &(t, m)| {
        let bench_system = DispatcherBenchSystem::new();
        let (refs, counters) = spawn_team(&bench_system.system, t, BALANCING_DISPATCHER_ID);
        b.iter(|| flood_and_wait(&refs[0], &counters, m));
        bench_system.terminate();
      });
    }
  }

  group.finish();
}

fn bench_default_dispatcher_baseline(c: &mut Criterion) {
  // 1:1 baseline using the default dispatcher: every message lands on the
  // single receiver mailbox and is drained sequentially. Provides the
  // reference wall clock against which the BalancingDispatcher numbers can
  // be interpreted (skew vs sequential, throughput delta, etc.).
  let mut group = c.benchmark_group("default_dispatcher_baseline");
  group.sample_size(10);
  group.measurement_time(Duration::from_secs(2));

  for batch in [100_usize, 1_000_usize] {
    group.throughput(Throughput::Elements(batch as u64));
    group.bench_with_input(format!("single_actor_batch_{batch}"), &batch, |b, &m| {
      let bench_system = DispatcherBenchSystem::new();
      let (refs, counters) = spawn_team(&bench_system.system, 1, DEFAULT_DISPATCHER_ID);
      b.iter(|| flood_and_wait(&refs[0], &counters, m));
      bench_system.terminate();
    });
  }

  group.finish();
}

criterion_group!(balancing_dispatcher, bench_balancing_dispatcher, bench_default_dispatcher_baseline);
criterion_main!(balancing_dispatcher);
