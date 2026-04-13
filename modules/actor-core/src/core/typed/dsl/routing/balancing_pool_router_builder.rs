//! Builder for configuring and constructing balancing pool routers.
//!
//! Implements Pekko's BalancingPool semantics using a shared work queue
//! and BehaviorInterceptor-based work-pull pattern, without modifying
//! the core/dispatch layer.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, collections::VecDeque, vec::Vec};

use fraktor_utils_core_rs::core::sync::{ArcShared, DefaultMutex, SharedLock};

use crate::core::{
  kernel::{
    actor::{Pid, error::ActorError},
    event::logging::LogLevel,
  },
  typed::{
    TypedActorRef,
    actor::TypedActorContext,
    behavior::{Behavior, BehaviorDirective},
    behavior_interceptor::BehaviorInterceptor,
    dsl::Behaviors,
    message_and_signals::BehaviorSignal,
    props::TypedProps,
  },
};

/// Shared work queue that the router enqueues messages into and idle
/// routees pull from.
///
/// Invariant: `!idle_workers.is_empty()` implies `pending.is_empty()`.
struct SharedWorkQueue<M>
where
  M: Send + Sync + Clone + 'static, {
  pending:      VecDeque<M>,
  idle_workers: Vec<TypedActorRef<M>>,
}

impl<M> SharedWorkQueue<M>
where
  M: Send + Sync + Clone + 'static,
{
  const fn new() -> Self {
    Self { pending: VecDeque::new(), idle_workers: Vec::new() }
  }

  fn take_worker_for_message(&mut self, message: M) -> Option<(TypedActorRef<M>, M)> {
    if let Some(worker) = self.idle_workers.pop() {
      return Some((worker, message));
    }
    self.pending.push_back(message);
    None
  }

  fn take_message_for_worker(&mut self, worker: TypedActorRef<M>) -> Option<(TypedActorRef<M>, M)> {
    if let Some(msg) = self.pending.pop_front() {
      Some((worker, msg))
    } else {
      self.idle_workers.push(worker);
      None
    }
  }

  /// Register a worker as idle. If pending work exists, dispatch it.
  fn register_idle(&mut self, worker: TypedActorRef<M>) -> Option<(TypedActorRef<M>, M)> {
    self.take_message_for_worker(worker)
  }

  /// Remove a worker from the idle list (e.g. on termination).
  fn remove_worker(&mut self, pid: &Pid) {
    if let Some(pos) = self.idle_workers.iter().position(|w| w.pid() == *pid) {
      self.idle_workers.remove(pos);
    }
  }
}

fn try_dispatch_or_requeue<M>(queue: &SharedLock<SharedWorkQueue<M>>, dispatch: Option<(TypedActorRef<M>, M)>)
where
  M: Send + Sync + Clone + 'static, {
  let Some((mut worker, message)) = dispatch else {
    return;
  };
  if worker.try_tell(message.clone()).is_err() {
    queue.with_lock(|queue| queue.pending.push_front(message));
  }
}

/// BehaviorInterceptor that implements work-pull for balancing pool routees.
///
/// After each message is processed by the inner behavior, the interceptor
/// checks the shared work queue for pending work and self-dispatches it.
/// On start, the routee registers itself as idle.
struct WorkPullInterceptor<M>
where
  M: Send + Sync + Clone + 'static, {
  queue: SharedLock<SharedWorkQueue<M>>,
}

impl<M> BehaviorInterceptor<M> for WorkPullInterceptor<M>
where
  M: Send + Sync + Clone + 'static,
{
  fn around_start(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    start: &mut dyn FnMut(&mut TypedActorContext<'_, M>) -> Result<Behavior<M>, ActorError>,
  ) -> Result<Behavior<M>, ActorError> {
    let result = start(ctx)?;
    // Only register as idle if the inner behavior did NOT return Stopped.
    // A routee that returns Stopped on start must not appear in the idle queue.
    if result.directive() != BehaviorDirective::Stopped {
      let dispatch = self.queue.with_lock(|queue| queue.register_idle(ctx.self_ref()));
      try_dispatch_or_requeue(&self.queue, dispatch);
    }
    Ok(result)
  }

  fn around_receive(
    &mut self,
    ctx: &mut TypedActorContext<'_, M>,
    message: &M,
    target: &mut dyn FnMut(&mut TypedActorContext<'_, M>, &M) -> Result<Behavior<M>, ActorError>,
  ) -> Result<Behavior<M>, ActorError> {
    let result = target(ctx, message)?;
    // Only re-register as idle if the routee is NOT stopping.
    // A routee that returned Stopped must not receive further work.
    if result.directive() != BehaviorDirective::Stopped {
      let dispatch = self.queue.with_lock(|queue| queue.register_idle(ctx.self_ref()));
      try_dispatch_or_requeue(&self.queue, dispatch);
    } else {
      // Remove from idle list in case it was previously registered.
      self.queue.with_lock(|queue| queue.remove_worker(&ctx.self_ref().pid()));
    }
    Ok(result)
  }
}

/// Configures and builds a balancing pool router behavior.
///
/// The resulting behavior spawns `pool_size` child actors that share a
/// single work queue. Messages sent to the router are enqueued into the
/// shared queue and dispatched to whichever routee becomes idle first.
///
/// This mirrors Pekko's `BalancingPool` semantics where all routees share
/// a mailbox and work is distributed via "work donating" (idle workers
/// pull from the shared queue).
///
/// Resizer is intentionally not supported, matching Pekko's constraint
/// (`resizer = None`).
pub struct BalancingPoolRouterBuilder<M>
where
  M: Send + Sync + Clone + 'static, {
  pool_size:        usize,
  behavior_factory: ArcShared<dyn Fn() -> Behavior<M> + Send + Sync>,
}

impl<M> BalancingPoolRouterBuilder<M>
where
  M: Send + Sync + Clone + 'static,
{
  /// Creates a new balancing pool router builder.
  ///
  /// # Panics
  ///
  /// Panics if `pool_size` is zero.
  pub(crate) fn new<F>(pool_size: usize, behavior_factory: F) -> Self
  where
    F: Fn() -> Behavior<M> + Send + Sync + 'static, {
    assert!(pool_size > 0, "pool size must be positive");
    Self { pool_size, behavior_factory: ArcShared::new(behavior_factory) }
  }

  /// Overrides the pool size.
  ///
  /// # Panics
  ///
  /// Panics if `pool_size` is zero.
  #[must_use]
  pub const fn with_pool_size(mut self, pool_size: usize) -> Self {
    assert!(pool_size > 0, "pool size must be positive");
    self.pool_size = pool_size;
    self
  }

  /// Builds the balancing pool router as a [`Behavior`].
  #[must_use]
  pub fn build(self) -> Behavior<M> {
    let pool_size = self.pool_size;
    let behavior_factory = self.behavior_factory;

    Behaviors::setup(move |ctx| {
      let queue = SharedLock::new_with_driver::<DefaultMutex<_>>(SharedWorkQueue::new());

      let mut routee_pids: Vec<Pid> = Vec::with_capacity(pool_size);
      for _ in 0..pool_size {
        let q = queue.clone();
        let bf = behavior_factory.clone();
        let props = TypedProps::<M>::from_behavior_factory(move || {
          let q2 = q.clone();
          let bf2 = bf.clone();
          Behaviors::intercept(
            move || Box::new(WorkPullInterceptor { queue: q2.clone() }),
            move || {
              let factory: &(dyn Fn() -> Behavior<M> + Send + Sync) = &*bf2;
              factory()
            },
          )
        });
        match ctx.spawn_child_watched(&props) {
          | Ok(child) => {
            routee_pids.push(child.actor_ref().pid());
          },
          | Err(e) => {
            let msg = alloc::format!("balancing pool router failed to spawn child: {:?}", e);
            ctx.system().emit_log(LogLevel::Warn, msg, Some(ctx.pid()), None);
            break;
          },
        }
      }

      // routee が1体も起動できなかった場合はルーターを停止する
      if routee_pids.is_empty() {
        ctx.system().emit_log(LogLevel::Error, "balancing pool router has no routees, stopping", Some(ctx.pid()), None);
        return Behaviors::stopped();
      }

      let queue_for_msg = queue.clone();
      let queue_for_sig = queue;
      let routee_pids = SharedLock::new_with_driver::<DefaultMutex<_>>(routee_pids);
      let routee_pids_for_sig = routee_pids;

      Behaviors::receive_message(move |_ctx, message: &M| {
        let dispatch = queue_for_msg.with_lock(|queue| queue.take_worker_for_message(message.clone()));
        try_dispatch_or_requeue(&queue_for_msg, dispatch);
        Ok(Behaviors::same())
      })
      .receive_signal(move |_ctx, signal| match signal {
        | BehaviorSignal::Terminated(pid) => {
          queue_for_sig.with_lock(|queue| queue.remove_worker(pid));
          let is_empty = routee_pids_for_sig.with_lock(|pids| {
            if let Some(pos) = pids.iter().position(|p| p == pid) {
              pids.remove(pos);
            }
            pids.is_empty()
          });
          if is_empty {
            return Ok(Behaviors::stopped());
          }
          Ok(Behaviors::same())
        },
        | _ => Ok(Behaviors::same()),
      })
    })
  }
}
