use alloc::{collections::BTreeMap, format, string::ToString, vec, vec::Vec};
use core::{
  future::pending,
  sync::atomic::{AtomicU32, Ordering},
  time::Duration,
};

use fraktor_utils_core_rs::{
  sync::{ArcShared, SpinSyncMutex},
  timing::delay::{DelayFuture, DelayProvider, ManualDelayProvider},
};
use tokio::sync::Barrier;

use super::*;
use crate::actor::{
  Actor, ActorCell, ActorContext, Pid,
  actor_ref::{ActorRef, NullSender},
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView, AskError},
  props::Props,
};

struct TerminationNoopActor;

impl Actor for TerminationNoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn termination_noop_actor_accepts_messages() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut ctx = ActorContext::new(&system, pid);
  let message = AnyMessage::new("termination probe");
  let mut actor = TerminationNoopActor;

  assert!(actor.receive(&mut ctx, message.as_view()).is_ok());
}

#[derive(Clone)]
struct SharedManualDelayProvider {
  inner: ArcShared<SpinSyncMutex<ManualDelayProvider>>,
}

impl SharedManualDelayProvider {
  fn new() -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(ManualDelayProvider::new())) }
  }

  fn trigger_all(&self) {
    self.inner.lock().trigger_all();
  }
}

impl DelayProvider for SharedManualDelayProvider {
  fn delay(&mut self, duration: Duration) -> DelayFuture {
    self.inner.lock().delay(duration)
  }
}

fn default_shutdown() -> CoordinatedShutdown {
  CoordinatedShutdown::with_default_phases().expect("default phases should be valid")
}

#[test]
fn default_phases_are_topologically_ordered() {
  let cs = default_shutdown();
  let ordered = cs.ordered_phases();
  let idx = |name: &str| ordered.iter().position(|n| n == name).unwrap();

  assert!(idx(CoordinatedShutdown::PHASE_BEFORE_SERVICE_UNBIND) < idx(CoordinatedShutdown::PHASE_SERVICE_UNBIND));
  assert!(idx(CoordinatedShutdown::PHASE_SERVICE_UNBIND) < idx(CoordinatedShutdown::PHASE_SERVICE_REQUESTS_DONE));
  assert!(idx(CoordinatedShutdown::PHASE_SERVICE_REQUESTS_DONE) < idx(CoordinatedShutdown::PHASE_SERVICE_STOP));
  assert!(idx(CoordinatedShutdown::PHASE_SERVICE_STOP) < idx(CoordinatedShutdown::PHASE_BEFORE_CLUSTER_SHUTDOWN));
  assert!(idx(CoordinatedShutdown::PHASE_BEFORE_CLUSTER_SHUTDOWN) < idx(CoordinatedShutdown::PHASE_CLUSTER_LEAVE));
  assert!(idx(CoordinatedShutdown::PHASE_CLUSTER_LEAVE) < idx(CoordinatedShutdown::PHASE_CLUSTER_SHUTDOWN));
  assert!(
    idx(CoordinatedShutdown::PHASE_CLUSTER_SHUTDOWN) < idx(CoordinatedShutdown::PHASE_BEFORE_ACTOR_SYSTEM_TERMINATE)
  );
  assert!(
    idx(CoordinatedShutdown::PHASE_BEFORE_ACTOR_SYSTEM_TERMINATE)
      < idx(CoordinatedShutdown::PHASE_ACTOR_SYSTEM_TERMINATE)
  );
}

#[test]
fn add_task_rejects_unknown_phase() {
  let cs = default_shutdown();
  let result = cs.add_task("nonexistent-phase", "my-task", || async {});
  assert!(matches!(result, Err(CoordinatedShutdownError::UnknownPhase(_))));
}

#[test]
fn add_task_rejects_empty_name() {
  let cs = default_shutdown();
  let result = cs.add_task(CoordinatedShutdown::PHASE_SERVICE_STOP, "", || async {});
  assert!(matches!(result, Err(CoordinatedShutdownError::EmptyTaskName)));
}

#[test]
fn timeout_returns_configured_value() {
  let cs = default_shutdown();
  let timeout = cs.timeout(CoordinatedShutdown::PHASE_SERVICE_STOP).unwrap();
  assert_eq!(timeout, DEFAULT_PHASE_TIMEOUT);
}

#[test]
fn timeout_rejects_unknown_phase() {
  let cs = default_shutdown();
  let result = cs.timeout("nonexistent-phase");
  assert!(matches!(result, Err(CoordinatedShutdownError::UnknownPhase(_))));
}

#[test]
fn total_timeout_is_zero_without_tasks() {
  let cs = default_shutdown();
  assert_eq!(cs.total_timeout(), Duration::ZERO);
}

#[test]
fn total_timeout_sums_phases_with_tasks() {
  let cs = default_shutdown();
  cs.add_task(CoordinatedShutdown::PHASE_SERVICE_STOP, "t1", || async {}).unwrap();
  cs.add_task(CoordinatedShutdown::PHASE_SERVICE_UNBIND, "t2", || async {}).unwrap();
  assert_eq!(cs.total_timeout(), DEFAULT_PHASE_TIMEOUT * 2);
}

#[tokio::test]
async fn add_cancellable_task_skips_cancelled_task() {
  let cs = default_shutdown();
  let handle = cs.add_cancellable_task(CoordinatedShutdown::PHASE_SERVICE_STOP, "cancellable", || async {}).unwrap();

  assert!(handle.cancel());
  cs.run(CoordinatedShutdownReason::Unknown).await;

  assert!(handle.is_cancelled());
  assert!(!handle.is_completed());
}

#[tokio::test]
async fn add_cancellable_task_marks_completed_after_run() {
  let cs = default_shutdown();
  let counter = ArcShared::new(AtomicU32::new(0));
  let c = counter.clone();
  let handle = cs
    .add_cancellable_task(CoordinatedShutdown::PHASE_SERVICE_STOP, "cancellable", move || async move {
      c.fetch_add(1, Ordering::SeqCst);
    })
    .unwrap();

  cs.run(CoordinatedShutdownReason::Unknown).await;

  assert!(handle.is_completed());
  assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn add_cancellable_task_rejects_invalid_inputs() {
  let cs = default_shutdown();

  assert!(matches!(
    cs.add_cancellable_task("missing-phase", "task", || async {}),
    Err(CoordinatedShutdownError::UnknownPhase(_))
  ));
  assert!(matches!(
    cs.add_cancellable_task(CoordinatedShutdown::PHASE_SERVICE_STOP, "", || async {}),
    Err(CoordinatedShutdownError::EmptyTaskName)
  ));
}

#[tokio::test]
async fn add_cancellable_task_rejects_after_run_started() {
  let cs = default_shutdown();
  cs.run(CoordinatedShutdownReason::Unknown).await;

  let result = cs.add_cancellable_task(CoordinatedShutdown::PHASE_SERVICE_STOP, "late", || async {});

  assert!(matches!(result, Err(CoordinatedShutdownError::RunAlreadyStarted)));
}

#[tokio::test]
async fn add_actor_termination_task_completes_for_absent_actor() {
  let system = ActorSystem::new_empty();
  let actor = ActorRef::with_system(Pid::new(42, 0), NullSender, &system.state());
  let cs = default_shutdown();
  let marker = ArcShared::new(AtomicU32::new(0));
  let m = marker.clone();

  cs.add_actor_termination_task(CoordinatedShutdown::PHASE_SERVICE_STOP, "actor-termination", actor, None).unwrap();
  cs.add_task(CoordinatedShutdown::PHASE_SERVICE_STOP, "marker", move || async move {
    m.fetch_add(1, Ordering::SeqCst);
  })
  .unwrap();
  cs.run(CoordinatedShutdownReason::Unknown).await;

  assert_eq!(marker.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn wait_until_actor_disappears_times_out_when_cell_remains_registered() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| TerminationNoopActor);
  let cell = ActorCell::create(system.state(), pid, None, "termination-wait".to_string(), &props).expect("create cell");
  system.state().register_cell(cell);
  let actor = ActorRef::with_system(pid, NullSender, &system.state());

  let wait = tokio::spawn(CoordinatedShutdown::wait_until_actor_disappears(actor, Duration::from_millis(1)));
  tokio::task::yield_now().await;
  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));

  assert!(matches!(wait.await.expect("wait task"), Err(AskError::Timeout)));
}

#[tokio::test]
async fn wait_until_actor_disappears_returns_ok_without_system_state() {
  let actor = ActorRef::new_with_builtin_lock(Pid::new(44, 0), NullSender);

  let result = CoordinatedShutdown::wait_until_actor_disappears(actor, Duration::from_millis(1)).await;

  assert!(result.is_ok());
}

#[tokio::test]
async fn wait_until_actor_disappears_returns_ok_after_cell_is_removed() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| TerminationNoopActor);
  let cell =
    ActorCell::create(system.state(), pid, None, "termination-wait-remove".to_string(), &props).expect("create cell");
  system.state().register_cell(cell);
  let actor = ActorRef::with_system(pid, NullSender, &system.state());

  let wait = tokio::spawn(CoordinatedShutdown::wait_until_actor_disappears(actor, Duration::from_millis(2)));
  tokio::task::yield_now().await;
  system.state().remove_cell(&pid);
  system.scheduler().with_write(|scheduler| scheduler.run_for_test(1));

  assert!(wait.await.expect("wait task").is_ok());
}

#[tokio::test]
async fn add_actor_termination_task_swallows_wait_errors() {
  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let props = Props::from_fn(|| TerminationNoopActor);
  let cell =
    ActorCell::create(system.state(), pid, None, "termination-best-effort".to_string(), &props).expect("create cell");
  system.state().register_cell(cell);
  let actor = ActorRef::with_system(pid, NullSender, &system.state());
  let mut phases = BTreeMap::new();
  phases.insert("fast-phase".to_string(), CoordinatedShutdownPhase::new(vec![], Duration::ZERO));
  let cs = CoordinatedShutdown::new(phases).expect("shutdown");

  cs.add_actor_termination_task("fast-phase", "actor-termination", actor, None).unwrap();
  cs.run(CoordinatedShutdownReason::Unknown).await;

  assert!(cs.is_running());
  assert!(system.state().cell(&pid).is_some());
}

#[tokio::test]
async fn add_actor_termination_task_accepts_stop_message() {
  let system = ActorSystem::new_empty();
  let actor = ActorRef::with_system(Pid::new(43, 0), NullSender, &system.state());
  let cs = default_shutdown();

  cs.add_actor_termination_task(
    CoordinatedShutdown::PHASE_SERVICE_STOP,
    "actor-termination",
    actor,
    Some(AnyMessage::new("stop")),
  )
  .unwrap();
  cs.run(CoordinatedShutdownReason::Unknown).await;

  assert!(cs.is_running());
}

#[test]
fn shutdown_reason_is_none_before_run() {
  let cs = default_shutdown();
  assert!(cs.shutdown_reason().is_none());
  assert!(!cs.is_running());
}

#[test]
fn cyclic_dependency_detected() {
  let mut phases = BTreeMap::new();
  phases.insert("a".to_string(), CoordinatedShutdownPhase::new(vec!["b".to_string()], Duration::from_secs(1)));
  phases.insert("b".to_string(), CoordinatedShutdownPhase::new(vec!["a".to_string()], Duration::from_secs(1)));
  let result = CoordinatedShutdown::new(phases);
  assert!(matches!(result, Err(CoordinatedShutdownError::CyclicDependency(_))));
}

#[tokio::test]
async fn run_executes_tasks_in_phase_order() {
  let cs = default_shutdown();
  let order = ArcShared::new(SpinSyncMutex::new(Vec::<i32>::new()));

  let o1 = order.clone();
  cs.add_task(CoordinatedShutdown::PHASE_SERVICE_STOP, "stop-task", move || async move {
    o1.lock().push(2);
  })
  .unwrap();

  let o2 = order.clone();
  cs.add_task(CoordinatedShutdown::PHASE_SERVICE_UNBIND, "unbind-task", move || async move {
    o2.lock().push(1);
  })
  .unwrap();

  cs.run(CoordinatedShutdownReason::ActorSystemTerminate).await;

  let recorded = order.lock().clone();
  assert_eq!(recorded, vec![1, 2]);
}

#[tokio::test]
async fn run_is_idempotent() {
  let cs = default_shutdown();
  let counter = ArcShared::new(AtomicU32::new(0));

  let c = counter.clone();
  cs.add_task(CoordinatedShutdown::PHASE_SERVICE_STOP, "count-task", move || async move {
    c.fetch_add(1, Ordering::SeqCst);
  })
  .unwrap();

  cs.run(CoordinatedShutdownReason::ActorSystemTerminate).await;
  cs.run(CoordinatedShutdownReason::Unknown).await;

  assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn run_records_reason() {
  let cs = default_shutdown();
  cs.run(CoordinatedShutdownReason::ProcessSignal).await;
  assert_eq!(cs.shutdown_reason(), Some(CoordinatedShutdownReason::ProcessSignal));
  assert!(cs.is_running());
}

#[tokio::test]
async fn run_skips_disabled_phases() {
  let mut phases = BTreeMap::new();
  phases.insert("phase-a".to_string(), CoordinatedShutdownPhase::new(vec![], Duration::from_secs(1)));
  phases.insert(
    "phase-b".to_string(),
    CoordinatedShutdownPhase::new(vec!["phase-a".to_string()], Duration::from_secs(1)).with_enabled(false),
  );

  let cs = CoordinatedShutdown::new(phases).unwrap();
  let executed = ArcShared::new(AtomicU32::new(0));

  let e = executed.clone();
  cs.add_task("phase-b", "disabled-task", move || async move {
    e.fetch_add(1, Ordering::SeqCst);
  })
  .unwrap();

  cs.run(CoordinatedShutdownReason::Unknown).await;
  assert_eq!(executed.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn tasks_within_phase_run_concurrently() {
  let cs = default_shutdown();
  let counter = ArcShared::new(AtomicU32::new(0));
  let barrier = ArcShared::new(Barrier::new(3));

  for i in 0..3 {
    let c = counter.clone();
    let b = barrier.clone();
    cs.add_task(CoordinatedShutdown::PHASE_SERVICE_STOP, &format!("task-{i}"), move || async move {
      // 全タスクがランデブーポイントに到達するまで待機（並行実行の証明）
      b.wait().await;
      c.fetch_add(1, Ordering::SeqCst);
    })
    .unwrap();
  }

  cs.run(CoordinatedShutdownReason::Unknown).await;
  assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn phase_timeout_is_respected() {
  let mut phases = BTreeMap::new();
  phases.insert("fast-phase".to_string(), CoordinatedShutdownPhase::new(vec![], Duration::from_millis(50)));

  let delay_provider = SharedManualDelayProvider::new();
  let cs = ArcShared::new(CoordinatedShutdown::new_with_delay_provider(phases, delay_provider.clone()).unwrap());
  let completed = ArcShared::new(AtomicU32::new(0));

  let c = completed.clone();
  cs.add_task("fast-phase", "slow-task", move || async move {
    pending::<()>().await;
    c.fetch_add(1, Ordering::SeqCst);
  })
  .unwrap();

  let cs_for_run = cs.clone();
  let run_task = tokio::spawn(async move {
    cs_for_run.run(CoordinatedShutdownReason::Unknown).await;
  });

  tokio::task::yield_now().await;
  delay_provider.trigger_all();
  run_task.await.expect("run task");

  assert_eq!(completed.load(Ordering::SeqCst), 0);
}

#[test]
fn reason_display() {
  assert_eq!(CoordinatedShutdownReason::Unknown.to_string(), "UnknownReason");
  assert_eq!(CoordinatedShutdownReason::ActorSystemTerminate.to_string(), "ActorSystemTerminateReason");
  assert_eq!(CoordinatedShutdownReason::ProcessSignal.to_string(), "ProcessSignalReason");
  assert_eq!(CoordinatedShutdownReason::Custom("my-reason".to_string()).to_string(), "Custom(my-reason)");
}

#[test]
fn error_display() {
  let err = CoordinatedShutdownError::UnknownPhase("xyz".to_string());
  assert_eq!(err.to_string(), "unknown phase [xyz]");

  let err = CoordinatedShutdownError::EmptyTaskName;
  assert_eq!(err.to_string(), "task name must not be empty");

  let err = CoordinatedShutdownError::CyclicDependency("a".to_string());
  assert!(err.to_string().contains("cycle detected"));
}

/// `get` returns `None` when the extension has not been registered.
#[test]
fn get_returns_none_when_extension_not_registered() {
  let system = ActorSystem::new_empty();
  let result = CoordinatedShutdown::get(&system);
  assert!(result.is_none(), "should return None when extension is not registered");
}

/// `get` returns `Some` after the extension has been registered via `ExtendedActorSystem`.
#[test]
fn get_returns_some_after_extension_registered() {
  let system = ActorSystem::new_empty();
  let extended = system.extended();
  extended.register_extension(&CoordinatedShutdownId);

  let result = CoordinatedShutdown::get(&system);
  assert!(result.is_some(), "should return Some after extension is registered");
}

/// `get` returns a functional `CoordinatedShutdown` instance that supports adding tasks.
#[test]
fn get_returns_functional_instance() {
  let system = ActorSystem::new_empty();
  let extended = system.extended();
  extended.register_extension(&CoordinatedShutdownId);

  let cs = CoordinatedShutdown::get(&system).expect("extension should be available");
  let result = cs.add_task(CoordinatedShutdown::PHASE_SERVICE_STOP, "test-task", || async {});
  assert!(result.is_ok(), "should be able to add tasks to the retrieved instance");
}
