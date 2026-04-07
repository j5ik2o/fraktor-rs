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
  num::NonZeroUsize,
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

use crate::std::{default_tick_driver_config, dispatch::dispatcher::TokioExecutor};

const BALANCING_DISPATCHER_ID: &str = "balancing-contention";
const SPAWN_TIMEOUT: Duration = Duration::from_secs(5);
const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

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
  let handle = tokio::runtime::Handle::current();

  // High throughput so a single mailbox.run() can drain the entire batch.
  // The default `with_defaults` value is 5, which is too low for the
  // contention scenario: with throughput=5 and batch=1000, the receiver
  // mailbox would have to be re-scheduled 200 times. The current dispatcher
  // tree relies on follow-up `tell()` calls to trigger re-schedule via
  // `register_for_execution`, so once all `tell()` calls have already been
  // submitted and the first drain returns under the throughput limit, the
  // remaining envelopes can sit in the queue without being woken up. This
  // is a separate finding (lost wake-up) that is out of scope for the
  // Phase 14.5 contention observation; bumping the throughput here lets the
  // contention test focus on the documented "no active wake of idle team
  // members → skew" trade-off rather than the lost wake-up bug.
  let throughput = NonZeroUsize::new(4096).expect("non-zero throughput");

  let default_settings = DispatcherSettings::with_defaults(DEFAULT_DISPATCHER_ID).with_throughput(throughput);
  let default_executor = ExecutorShared::new(TokioExecutor::new(handle.clone()));
  let default_configurator: Box<dyn MessageDispatcherConfigurator> =
    Box::new(DefaultDispatcherConfigurator::new(&default_settings, default_executor));

  let balancing_settings = DispatcherSettings::with_defaults(BALANCING_DISPATCHER_ID).with_throughput(throughput);
  let balancing_executor = ExecutorShared::new(TokioExecutor::new(handle));
  let balancing_configurator: Box<dyn MessageDispatcherConfigurator> =
    Box::new(BalancingDispatcherConfigurator::new(&balancing_settings, balancing_executor));

  let config = ActorSystemConfig::default()
    .with_tick_driver(default_tick_driver_config())
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
  let deadline = started + DRAIN_TIMEOUT;
  while total.load(Ordering::Acquire) < count {
    if Instant::now() > deadline {
      panic!(
        "timed out waiting for drain: total={} expected={}",
        total.load(Ordering::Acquire),
        count
      );
    }
    std::thread::yield_now();
  }
  started.elapsed()
}

fn snapshot(per_actor: &[Arc<AtomicUsize>]) -> Vec<usize> {
  per_actor.iter().map(|c| c.load(Ordering::Acquire)).collect()
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
  // Invariants asserted:
  // - Every envelope is processed exactly once (sum == batch).
  // - All four team members observe work (`min > 0` → no idle members).
  //   This is empirically stable across the runs we sampled.
  //
  // Observed samples (5 consecutive runs on a multi-thread Tokio runtime,
  // worker_threads=4, batch=1000, throughput=4096):
  //   per_actor=[444, 288, 166, 102]  elapsed=21ms  idle=0
  //   per_actor=[244, 249, 240, 267]  elapsed=3ms   idle=0
  //   per_actor=[428, 165, 225, 182]  elapsed=3ms   idle=0
  //   per_actor=[295, 260, 229, 216]  elapsed=3ms   idle=0
  //   per_actor=[240, 259, 239, 262]  elapsed=3ms   idle=0
  //
  // Findings:
  // - design.md §9 predicted that V1 (no active wake of idle team members)
  //   could leave team members idle while the receiver drains. In practice
  //   the natural drain pattern keeps all four members working: even when
  //   one team member dominates the distribution (max/min ≈ 4.4 in the
  //   most skewed run), every member processes a non-trivial share of the
  //   batch. No run produced an idle team member.
  // - The receiver (actor[0]) tends to skew slightly higher because the
  //   tells land on its own mailbox before any sibling can pick them up.
  //
  // Out of scope for Phase 14.5 (logged for follow-up):
  // - At the default `throughput=5`, the same scenario hangs after ~450
  //   envelopes because the mailbox `set_idle()` return value is currently
  //   ignored in `Mailbox::run`, so the dispatcher relies on follow-up
  //   `tell()` calls to re-schedule. When all `tell()`s have already been
  //   submitted, the remaining envelopes can sit in the queue without a
  //   wake-up. This test sidesteps the issue by raising the throughput so
  //   a single drain handles the entire batch — the lost wake-up bug
  //   itself is a separate finding worth a dedicated change.
  const TEAM_SIZE: usize = 4;
  const BATCH: usize = 1000;

  let system = tokio::task::spawn_blocking(build_system).await.expect("build system");
  let (refs, per_actor, total) =
    tokio::task::spawn_blocking({
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
