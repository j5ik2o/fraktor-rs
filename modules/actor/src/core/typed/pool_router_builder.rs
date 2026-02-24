//! Builder for configuring and constructing pool routers.

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use crate::core::{
  event::logging::LogLevel,
  typed::{
    Behaviors, actor::TypedActorRefGeneric, behavior::Behavior, behavior_signal::BehaviorSignal,
    props::TypedPropsGeneric,
  },
};

/// Configures and builds a pool router behavior.
///
/// The resulting behavior spawns `pool_size` child actors and distributes
/// incoming messages to them using round-robin routing.
pub struct PoolRouterBuilderGeneric<M, TB = NoStdToolbox>
where
  M: Send + Sync + Clone + 'static,
  TB: RuntimeToolbox + 'static, {
  pool_size:        usize,
  behavior_factory: ArcShared<dyn Fn() -> Behavior<M, TB> + Send + Sync>,
}

/// Type alias for [`PoolRouterBuilderGeneric`] with the default [`NoStdToolbox`].
pub type PoolRouterBuilder<M> = PoolRouterBuilderGeneric<M, NoStdToolbox>;

impl<M, TB> PoolRouterBuilderGeneric<M, TB>
where
  M: Send + Sync + Clone + 'static,
  TB: RuntimeToolbox + 'static,
{
  /// Creates a new pool router builder with the given factory.
  ///
  /// # Panics
  ///
  /// Panics if `pool_size` is zero.
  pub(crate) fn new<F>(pool_size: usize, behavior_factory: F) -> Self
  where
    F: Fn() -> Behavior<M, TB> + Send + Sync + 'static, {
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

  /// Builds the pool router as a [`Behavior`].
  #[must_use]
  #[allow(clippy::redundant_closure)]
  pub fn build(self) -> Behavior<M, TB> {
    let pool_size = self.pool_size;
    let behavior_factory = self.behavior_factory;

    Behaviors::setup(move |ctx| {
      let bf = behavior_factory.clone();
      let props = TypedPropsGeneric::<M, TB>::from_behavior_factory(move || bf());

      let mut routee_vec: Vec<TypedActorRefGeneric<M, TB>> = Vec::with_capacity(pool_size);
      for _ in 0..pool_size {
        match ctx.spawn_child_watched(&props) {
          | Ok(child) => routee_vec.push(child.actor_ref().clone()),
          | Err(e) => {
            let msg = alloc::format!("pool router failed to spawn child: {:?}", e);
            ctx.system().emit_log(LogLevel::Warn, msg, Some(ctx.pid()));
            break;
          },
        }
      }

      let mutex = <TB::MutexFamily as SyncMutexFamily>::create(routee_vec);
      let routees: ArcShared<ToolboxMutex<Vec<TypedActorRefGeneric<M, TB>>, TB>> = ArcShared::new(mutex);
      let routees_for_msg = routees.clone();
      let routees_for_sig = routees;
      let index = AtomicUsize::new(0);

      Behaviors::receive_message(move |_ctx, message: &M| {
        let guard = routees_for_msg.lock();
        if guard.is_empty() {
          return Ok(Behaviors::same());
        }
        let idx = index.fetch_add(1, Ordering::Relaxed) % guard.len();
        let mut target = guard[idx].clone();
        drop(guard);
        let _ = target.tell(message.clone());
        Ok(Behaviors::same())
      })
      .receive_signal(move |_ctx, signal| match signal {
        | BehaviorSignal::Terminated(pid) => {
          let mut guard = routees_for_sig.lock();
          guard.retain(|r| r.pid() != *pid);
          if guard.is_empty() {
            return Ok(Behaviors::stopped());
          }
          Ok(Behaviors::same())
        },
        | _ => Ok(Behaviors::same()),
      })
    })
  }
}
