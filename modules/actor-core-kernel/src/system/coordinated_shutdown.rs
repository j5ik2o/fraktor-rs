//! Coordinated shutdown orchestration with phased task execution.

use alloc::{
  boxed::Box,
  collections::{BTreeMap, BTreeSet},
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::{future::Future, pin::Pin, task::Poll, time::Duration};

use fraktor_utils_core_rs::{
  sync::{ArcShared, DefaultMutex, SharedAccess, SharedLock, SyncOnce},
  timing::delay::DelayProvider,
};
use futures::future::{Either, join_all, poll_fn, select};
use portable_atomic::{AtomicBool, AtomicU64, Ordering};

use super::{coordinated_shutdown_error::CoordinatedShutdownError, coordinated_shutdown_id::CoordinatedShutdownId};
use crate::{
  actor::{
    actor_ref::ActorRef,
    extension::Extension,
    messaging::{AnyMessage, AskError},
    scheduler::SchedulerHandle,
  },
  pattern::graceful_stop_with_message,
  system::{ActorSystem, CoordinatedShutdownPhase, CoordinatedShutdownReason},
};

#[cfg(test)]
#[path = "coordinated_shutdown_test.rs"]
mod tests;

/// Async task closure type for shutdown phases.
type ShutdownTaskFn = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// A shutdown task.
struct ShutdownTask {
  task:   ShutdownTaskFn,
  handle: Option<SchedulerHandle>,
}

impl ShutdownTask {
  const fn new(task: ShutdownTaskFn) -> Self {
    Self { task, handle: None }
  }

  const fn cancellable(task: ShutdownTaskFn, handle: SchedulerHandle) -> Self {
    Self { task, handle: Some(handle) }
  }

  fn into_future(self) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    let Some(handle) = self.handle else {
      return (self.task)();
    };
    Box::pin(async move {
      let entry = handle.entry();
      if entry.is_cancelled() || !entry.try_begin_execute() {
        return;
      }
      (self.task)().await;
      if !entry.is_cancelled() {
        entry.mark_completed();
      }
    })
  }
}

/// Default timeout for phases when not explicitly configured.
const DEFAULT_PHASE_TIMEOUT: Duration = Duration::from_secs(5);

/// Coordinated shutdown with phased task execution.
///
/// Provides an ordered shutdown sequence where tasks are grouped into phases.
/// Phases execute in topological order (respecting dependencies), and tasks
/// within a single phase run concurrently.
///
/// Calling [`run`](Self::run) multiple times is safe — only the first invocation
/// triggers the shutdown sequence.
///
/// # Phase constants
///
/// The following pre-defined phases are available (in execution order):
///
/// 1. [`PHASE_BEFORE_SERVICE_UNBIND`](Self::PHASE_BEFORE_SERVICE_UNBIND)
/// 2. [`PHASE_SERVICE_UNBIND`](Self::PHASE_SERVICE_UNBIND)
/// 3. [`PHASE_SERVICE_REQUESTS_DONE`](Self::PHASE_SERVICE_REQUESTS_DONE)
/// 4. [`PHASE_SERVICE_STOP`](Self::PHASE_SERVICE_STOP)
/// 5. [`PHASE_BEFORE_CLUSTER_SHUTDOWN`](Self::PHASE_BEFORE_CLUSTER_SHUTDOWN)
/// 6. [`PHASE_CLUSTER_LEAVE`](Self::PHASE_CLUSTER_LEAVE)
/// 7. [`PHASE_CLUSTER_SHUTDOWN`](Self::PHASE_CLUSTER_SHUTDOWN)
/// 8. [`PHASE_BEFORE_ACTOR_SYSTEM_TERMINATE`](Self::PHASE_BEFORE_ACTOR_SYSTEM_TERMINATE)
/// 9. [`PHASE_ACTOR_SYSTEM_TERMINATE`](Self::PHASE_ACTOR_SYSTEM_TERMINATE)
pub struct CoordinatedShutdown {
  phases:         BTreeMap<String, CoordinatedShutdownPhase>,
  ordered:        Vec<String>,
  known_phases:   BTreeSet<String>,
  tasks:          SharedLock<BTreeMap<String, Vec<ShutdownTask>>>,
  run_started:    AtomicBool,
  run_done:       AtomicBool,
  reason:         SyncOnce<CoordinatedShutdownReason>,
  delay_provider: Option<SharedLock<Box<dyn DelayProvider>>>,
  next_task_id:   AtomicU64,
}

impl CoordinatedShutdown {
  /// Phase for actor system termination (the last phase).
  pub const PHASE_ACTOR_SYSTEM_TERMINATE: &'static str = "actor-system-terminate";
  /// Phase for application tasks before the actor system terminates.
  pub const PHASE_BEFORE_ACTOR_SYSTEM_TERMINATE: &'static str = "before-actor-system-terminate";
  /// Phase for application tasks before the cluster shuts down.
  pub const PHASE_BEFORE_CLUSTER_SHUTDOWN: &'static str = "before-cluster-shutdown";
  /// Phase for application tasks before services are unbound.
  pub const PHASE_BEFORE_SERVICE_UNBIND: &'static str = "before-service-unbind";
  /// Phase for gracefully leaving the cluster.
  pub const PHASE_CLUSTER_LEAVE: &'static str = "cluster-leave";
  /// Phase for shutting down the cluster extension.
  pub const PHASE_CLUSTER_SHUTDOWN: &'static str = "cluster-shutdown";
  /// Phase that waits for in-progress requests to complete.
  pub const PHASE_SERVICE_REQUESTS_DONE: &'static str = "service-requests-done";
  /// Phase for the final service endpoint shutdown.
  pub const PHASE_SERVICE_STOP: &'static str = "service-stop";
  /// Phase that stops accepting new incoming requests.
  pub const PHASE_SERVICE_UNBIND: &'static str = "service-unbind";

  /// Returns the coordinated shutdown extension from the actor system.
  ///
  /// This is the primary entry point corresponding to Pekko's
  /// `CoordinatedShutdown.get(system)`.
  ///
  /// Returns `None` if the extension has not been registered.
  #[must_use]
  pub fn get(system: &ActorSystem) -> Option<ArcShared<CoordinatedShutdown>> {
    system.extended().extension(&CoordinatedShutdownId)
  }

  /// Creates a new coordinated shutdown with the default phase graph.
  ///
  /// # Errors
  ///
  /// Returns [`CoordinatedShutdownError::CyclicDependency`] if the default
  /// phase graph contains a cycle (should not happen with the built-in phases).
  pub fn with_default_phases() -> Result<Self, CoordinatedShutdownError> {
    Self::with_default_phases_and_delay_provider(None)
  }

  /// Creates a new coordinated shutdown with the default phase graph and
  /// a delay provider used for phase timeouts.
  ///
  /// # Errors
  ///
  /// Returns [`CoordinatedShutdownError::CyclicDependency`] if the default
  /// phase graph contains a cycle (should not happen with the built-in phases).
  pub fn with_default_phases_with_delay_provider(
    delay_provider: impl DelayProvider,
  ) -> Result<Self, CoordinatedShutdownError> {
    Self::with_default_phases_and_delay_provider(Some(Box::new(delay_provider)))
  }

  fn with_default_phases_and_delay_provider(
    delay_provider: Option<Box<dyn DelayProvider>>,
  ) -> Result<Self, CoordinatedShutdownError> {
    let mut phases = BTreeMap::new();
    phases.insert(
      Self::PHASE_BEFORE_SERVICE_UNBIND.to_string(),
      CoordinatedShutdownPhase::new(vec![], DEFAULT_PHASE_TIMEOUT),
    );
    phases.insert(
      Self::PHASE_SERVICE_UNBIND.to_string(),
      CoordinatedShutdownPhase::new(vec![Self::PHASE_BEFORE_SERVICE_UNBIND.to_string()], DEFAULT_PHASE_TIMEOUT),
    );
    phases.insert(
      Self::PHASE_SERVICE_REQUESTS_DONE.to_string(),
      CoordinatedShutdownPhase::new(vec![Self::PHASE_SERVICE_UNBIND.to_string()], DEFAULT_PHASE_TIMEOUT),
    );
    phases.insert(
      Self::PHASE_SERVICE_STOP.to_string(),
      CoordinatedShutdownPhase::new(vec![Self::PHASE_SERVICE_REQUESTS_DONE.to_string()], DEFAULT_PHASE_TIMEOUT),
    );
    phases.insert(
      Self::PHASE_BEFORE_CLUSTER_SHUTDOWN.to_string(),
      CoordinatedShutdownPhase::new(vec![Self::PHASE_SERVICE_STOP.to_string()], DEFAULT_PHASE_TIMEOUT),
    );
    phases.insert(
      Self::PHASE_CLUSTER_SHUTDOWN.to_string(),
      CoordinatedShutdownPhase::new(vec![Self::PHASE_CLUSTER_LEAVE.to_string()], DEFAULT_PHASE_TIMEOUT),
    );
    phases.insert(
      Self::PHASE_CLUSTER_LEAVE.to_string(),
      CoordinatedShutdownPhase::new(vec![Self::PHASE_BEFORE_CLUSTER_SHUTDOWN.to_string()], DEFAULT_PHASE_TIMEOUT),
    );
    phases.insert(
      Self::PHASE_BEFORE_ACTOR_SYSTEM_TERMINATE.to_string(),
      CoordinatedShutdownPhase::new(vec![Self::PHASE_CLUSTER_SHUTDOWN.to_string()], DEFAULT_PHASE_TIMEOUT),
    );
    phases.insert(
      Self::PHASE_ACTOR_SYSTEM_TERMINATE.to_string(),
      CoordinatedShutdownPhase::new(vec![Self::PHASE_BEFORE_ACTOR_SYSTEM_TERMINATE.to_string()], DEFAULT_PHASE_TIMEOUT),
    );
    Self::new_with_optional_delay_provider(phases, delay_provider)
  }

  /// Creates a new coordinated shutdown with the provided phase definitions.
  ///
  /// # Errors
  ///
  /// Returns [`CoordinatedShutdownError::CyclicDependency`] if the phase
  /// dependency graph contains a cycle.
  pub fn new(phases: BTreeMap<String, CoordinatedShutdownPhase>) -> Result<Self, CoordinatedShutdownError> {
    Self::new_with_optional_delay_provider(phases, None)
  }

  /// Creates a new coordinated shutdown with the provided phase definitions
  /// and a delay provider used for phase timeouts.
  ///
  /// # Errors
  ///
  /// Returns [`CoordinatedShutdownError::CyclicDependency`] if the phase
  /// dependency graph contains a cycle.
  pub fn new_with_delay_provider(
    phases: BTreeMap<String, CoordinatedShutdownPhase>,
    delay_provider: impl DelayProvider,
  ) -> Result<Self, CoordinatedShutdownError> {
    Self::new_with_optional_delay_provider(phases, Some(Box::new(delay_provider)))
  }

  fn new_with_optional_delay_provider(
    phases: BTreeMap<String, CoordinatedShutdownPhase>,
    delay_provider: Option<Box<dyn DelayProvider>>,
  ) -> Result<Self, CoordinatedShutdownError> {
    // 全ての depends_on 参照先が定義済みフェーズであることを検証する
    for (name, phase) in &phases {
      for dep in phase.depends_on() {
        if !phases.contains_key(dep) {
          return Err(CoordinatedShutdownError::UnknownPhase(alloc::format!(
            "phase [{}] depends on undefined phase [{}]",
            name,
            dep
          )));
        }
      }
    }
    let ordered = Self::topological_sort(&phases)?;
    let known_phases = Self::collect_known_phases(&phases);
    Ok(Self {
      phases,
      ordered,
      known_phases,
      tasks: SharedLock::new_with_driver::<DefaultMutex<_>>(BTreeMap::new()),
      run_started: AtomicBool::new(false),
      run_done: AtomicBool::new(false),
      reason: SyncOnce::new(),
      delay_provider: delay_provider.map(SharedLock::new_with_driver::<DefaultMutex<_>>),
      next_task_id: AtomicU64::new(1),
    })
  }

  /// Adds a task to the specified phase.
  ///
  /// Tasks within a phase execute concurrently. The next phase starts only
  /// after all tasks of the current phase complete (or timeout).
  ///
  /// # Errors
  ///
  /// Returns an error if the phase is unknown or the task name is empty.
  pub fn add_task<F, Fut>(&self, phase: &str, task_name: &str, task: F) -> Result<(), CoordinatedShutdownError>
  where
    F: FnOnce() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    // シャットダウン開始後のタスク追加を拒否する（run() がフェーズを消費済みのため消失する）
    if self.run_started.load(Ordering::Acquire) {
      return Err(CoordinatedShutdownError::RunAlreadyStarted);
    }
    if !self.known_phases.contains(phase) {
      return Err(CoordinatedShutdownError::UnknownPhase(phase.to_string()));
    }
    if task_name.is_empty() {
      return Err(CoordinatedShutdownError::EmptyTaskName);
    }
    let shutdown_task = ShutdownTask::new(Box::new(move || Box::pin(task())));
    self.tasks.with_write(|tasks| {
      tasks.entry(phase.to_string()).or_default().push(shutdown_task);
    });
    Ok(())
  }

  /// Adds a cancellable task to the specified phase.
  ///
  /// The returned handle can be cancelled before the phase executes. Cancelled
  /// tasks remain registered but are skipped when the phase runs, mirroring the
  /// user-facing capability of Pekko `addCancellableTask`.
  ///
  /// # Errors
  ///
  /// Returns an error if the phase is unknown, the task name is empty, or the
  /// shutdown sequence has already started.
  pub fn add_cancellable_task<F, Fut>(
    &self,
    phase: &str,
    task_name: &str,
    task: F,
  ) -> Result<SchedulerHandle, CoordinatedShutdownError>
  where
    F: FnOnce() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    if self.run_started.load(Ordering::Acquire) {
      return Err(CoordinatedShutdownError::RunAlreadyStarted);
    }
    if !self.known_phases.contains(phase) {
      return Err(CoordinatedShutdownError::UnknownPhase(phase.to_string()));
    }
    if task_name.is_empty() {
      return Err(CoordinatedShutdownError::EmptyTaskName);
    }
    let handle = self.next_task_handle();
    handle.entry().mark_scheduled();
    let shutdown_task = ShutdownTask::cancellable(Box::new(move || Box::pin(task())), handle.clone());
    self.tasks.with_write(|tasks| {
      tasks.entry(phase.to_string()).or_default().push(shutdown_task);
    });
    Ok(handle)
  }

  /// Adds a task that waits for an actor to terminate.
  ///
  /// When `stop_message` is present, the task sends it before waiting for the
  /// actor to disappear from the system registry. When it is absent, the task
  /// only waits for an already initiated termination.
  ///
  /// # Errors
  ///
  /// Returns an error if the phase is unknown, the task name is empty, or the
  /// shutdown sequence has already started.
  pub fn add_actor_termination_task(
    &self,
    phase: &str,
    task_name: &str,
    actor: ActorRef,
    stop_message: Option<AnyMessage>,
  ) -> Result<(), CoordinatedShutdownError> {
    let timeout = self.timeout(phase)?;
    self.add_task(phase, task_name, move || async move {
      if let Err(_error) = Self::wait_for_actor_termination(actor, stop_message, timeout).await {
        // Coordinated shutdown tasks are best-effort and currently expose no per-task error
        // channel.
      }
    })
  }

  /// Returns the configured timeout for the given phase.
  ///
  /// # Errors
  ///
  /// Returns [`CoordinatedShutdownError::UnknownPhase`] if the phase is not defined.
  pub fn timeout(&self, phase: &str) -> Result<Duration, CoordinatedShutdownError> {
    self.phases.get(phase).map(|p| p.timeout()).ok_or_else(|| CoordinatedShutdownError::UnknownPhase(phase.to_string()))
  }

  /// Returns the total timeout across all phases that have registered tasks.
  #[must_use]
  pub fn total_timeout(&self) -> Duration {
    self.tasks.with_read(|tasks| {
      tasks
        .keys()
        .filter_map(|phase| self.phases.get(phase).map(|p| p.timeout()))
        .fold(Duration::ZERO, |acc, t| acc + t)
    })
  }

  /// Returns the shutdown reason if the shutdown has been started.
  #[must_use]
  pub fn shutdown_reason(&self) -> Option<CoordinatedShutdownReason> {
    self.reason.get().cloned()
  }

  /// Returns `true` if the shutdown sequence has been started.
  #[must_use]
  pub fn is_running(&self) -> bool {
    self.run_started.load(Ordering::Acquire)
  }

  /// Returns the ordered list of phase names.
  #[must_use]
  pub fn ordered_phases(&self) -> &[String] {
    &self.ordered
  }

  /// Runs the coordinated shutdown sequence.
  ///
  /// Executes all phases in topological order. Tasks within each phase run
  /// concurrently. Each phase is bounded by its configured timeout.
  ///
  /// This method is idempotent — calling it multiple times returns the same
  /// completion future and does not re-run the sequence.
  pub async fn run(&self, reason: CoordinatedShutdownReason) {
    if self.run_started.swap(true, Ordering::AcqRel) {
      poll_fn(|cx| {
        if self.run_done.load(Ordering::Acquire) {
          Poll::Ready(())
        } else {
          cx.waker().wake_by_ref();
          Poll::Pending
        }
      })
      .await;
      return;
    }

    // Future がドロップされても run_done を必ず true にするガード
    struct DoneGuard<'a>(&'a AtomicBool);
    impl Drop for DoneGuard<'_> {
      fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
      }
    }
    let _done_guard = DoneGuard(&self.run_done);

    self.reason.call_once(|| reason);

    for phase_name in &self.ordered {
      let Some(phase_config) = self.phases.get(phase_name) else {
        continue;
      };
      if !phase_config.enabled() {
        continue;
      }

      let phase_tasks = self.tasks.with_write(|tasks| tasks.remove(phase_name).unwrap_or_default());

      if phase_tasks.is_empty() {
        continue;
      }

      let recover = phase_config.recover();
      let timeout = phase_config.timeout();

      let phase_failed = self.run_phase_tasks(phase_tasks, timeout).await;

      if phase_failed && !recover {
        break;
      }
    }
    // run_done は DoneGuard の drop で設定される
  }

  /// Runs phase tasks concurrently and optionally awaits them with timeout.
  ///
  /// Returns `true` if the phase timed out.
  async fn run_phase_tasks(&self, tasks: Vec<ShutdownTask>, timeout: Duration) -> bool {
    let task_futures: Vec<Pin<Box<dyn Future<Output = ()> + Send>>> =
      tasks.into_iter().map(ShutdownTask::into_future).collect();
    let phase_future = Box::pin(async move {
      join_all(task_futures).await;
      false
    });

    let Some(delay_provider) = &self.delay_provider else {
      return phase_future.await;
    };

    let timeout_future = Box::pin(async {
      delay_provider.with_write(|provider| provider.delay(timeout)).await;
      true
    });

    match select(phase_future, timeout_future).await {
      | Either::Left((completed, _)) => completed,
      | Either::Right((timed_out, _)) => timed_out,
    }
  }

  fn collect_known_phases(phases: &BTreeMap<String, CoordinatedShutdownPhase>) -> BTreeSet<String> {
    phases.keys().cloned().collect()
  }

  fn next_task_handle(&self) -> SchedulerHandle {
    let raw = self.next_task_id.fetch_add(1, Ordering::Relaxed);
    SchedulerHandle::new(raw)
  }

  async fn wait_for_actor_termination(
    mut actor: ActorRef,
    stop_message: Option<AnyMessage>,
    timeout: Duration,
  ) -> Result<(), AskError> {
    let Some(stop_message) = stop_message else {
      return Self::wait_until_actor_disappears(actor, timeout).await;
    };
    graceful_stop_with_message(&mut actor, stop_message, timeout).await
  }

  async fn wait_until_actor_disappears(actor: ActorRef, timeout: Duration) -> Result<(), AskError> {
    let Some(system) = actor.system_state() else {
      return Ok(());
    };
    let pid = actor.pid();
    if system.cell(&pid).is_none() {
      return Ok(());
    }

    let mut remaining = timeout;
    let mut delay_provider = system.delay_provider();
    const TERMINATION_POLL_INTERVAL: Duration = Duration::from_millis(1);
    loop {
      if system.cell(&pid).is_none() {
        return Ok(());
      }
      if remaining.is_zero() {
        return Err(AskError::Timeout);
      }
      let delay_duration = core::cmp::min(remaining, TERMINATION_POLL_INTERVAL);
      delay_provider.delay(delay_duration).await;
      remaining = remaining.saturating_sub(delay_duration);
    }
  }

  /// Topological sort of phase dependencies (Kahn's algorithm).
  fn topological_sort(
    phases: &BTreeMap<String, CoordinatedShutdownPhase>,
  ) -> Result<Vec<String>, CoordinatedShutdownError> {
    let mut all_nodes = BTreeSet::new();
    let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
    let mut adjacency: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (name, phase) in phases {
      all_nodes.insert(name.clone());
      for dep in phase.depends_on() {
        all_nodes.insert(dep.clone());
      }
    }

    for node in &all_nodes {
      in_degree.entry(node.clone()).or_insert(0);
      adjacency.entry(node.clone()).or_default();
    }

    for (name, phase) in phases {
      for dep in phase.depends_on() {
        adjacency.entry(dep.clone()).or_default().push(name.clone());
        *in_degree.entry(name.clone()).or_insert(0) += 1;
      }
    }

    let mut queue: Vec<String> = in_degree.iter().filter(|&(_, deg)| *deg == 0).map(|(name, _)| name.clone()).collect();
    queue.sort();

    let mut result = Vec::new();

    while !queue.is_empty() {
      let node = queue.remove(0);
      result.push(node.clone());
      if let Some(neighbors) = adjacency.get(&node) {
        for neighbor in neighbors {
          if let Some(deg) = in_degree.get_mut(neighbor) {
            *deg -= 1;
            if *deg == 0 {
              queue.push(neighbor.clone());
              queue.sort();
            }
          }
        }
      }
    }

    if result.len() != all_nodes.len() {
      let remaining: Vec<_> = all_nodes.difference(&result.iter().cloned().collect()).cloned().collect();
      let cycle_node = remaining.first().cloned().unwrap_or_default();
      return Err(CoordinatedShutdownError::CyclicDependency(cycle_node));
    }

    Ok(result)
  }
}

// `ArcShared` requires `Sync` on inner types. `SharedLock` is `Send + Sync`,
// and `AtomicBool` is `Sync`, so the struct is `Send + Sync` by derivation.
impl Extension for CoordinatedShutdown {}
