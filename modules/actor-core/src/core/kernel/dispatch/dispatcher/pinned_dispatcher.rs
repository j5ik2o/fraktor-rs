//! Per-actor `MessageDispatcher` providing a dedicated execution lane.
//!
//! `PinnedDispatcher` enforces a 1:1 ownership model: only one actor at a time
//! can be registered with the dispatcher. The Pekko equivalent is
//! `org.apache.pekko.dispatch.PinnedDispatcher`.

#[cfg(test)]
mod tests;

use core::num::NonZeroUsize;

use fraktor_utils_core_rs::core::sync::ArcShared;

use super::{
  dispatcher_core::DispatcherCore, dispatcher_settings::DispatcherSettings, executor_shared::ExecutorShared,
  message_dispatcher::MessageDispatcher,
};
use crate::core::kernel::{
  actor::{ActorCell, Pid, spawn::SpawnError},
  runtime_lock_provider::{ActorRuntimeLockProvider, BuiltinSpinRuntimeLockProvider},
};

/// Dispatcher dedicated to a single actor.
///
/// Construction normalises throughput to `usize::MAX` and clears the throughput
/// deadline regardless of the supplied [`DispatcherSettings`], matching Pekko's
/// behaviour for `PinnedDispatcher`.
pub struct PinnedDispatcher {
  core:                   DispatcherCore,
  _runtime_lock_provider: ArcShared<dyn ActorRuntimeLockProvider>,
  owner:                  Option<Pid>,
}

impl PinnedDispatcher {
  /// Constructs a new `PinnedDispatcher` from the supplied settings and executor.
  ///
  /// The settings are normalised to `throughput = usize::MAX`,
  /// `throughput_deadline = None` before being handed to [`DispatcherCore`].
  #[must_use]
  pub fn new(settings: &DispatcherSettings, executor: ExecutorShared) -> Self {
    Self::new_with_provider(settings, executor, BuiltinSpinRuntimeLockProvider::shared())
  }

  /// Constructs a new `PinnedDispatcher` with the supplied runtime lock provider.
  #[must_use]
  pub fn new_with_provider(
    settings: &DispatcherSettings,
    executor: ExecutorShared,
    provider: ArcShared<dyn ActorRuntimeLockProvider>,
  ) -> Self {
    // SAFETY: `usize::MAX` is non-zero on every supported target.
    let max_throughput = unsafe { NonZeroUsize::new_unchecked(usize::MAX) };
    let normalised = settings.clone().with_throughput(max_throughput).with_throughput_deadline(None);
    Self {
      core:                   DispatcherCore::new(&normalised, executor),
      _runtime_lock_provider: provider,
      owner:                  None,
    }
  }

  /// Returns the currently registered owner pid, if any.
  #[must_use]
  pub const fn owner(&self) -> Option<Pid> {
    self.owner
  }
}

impl MessageDispatcher for PinnedDispatcher {
  fn core(&self) -> &DispatcherCore {
    &self.core
  }

  fn core_mut(&mut self) -> &mut DispatcherCore {
    &mut self.core
  }

  fn register_actor(&mut self, actor: &ArcShared<ActorCell>) -> Result<(), SpawnError> {
    let pid = actor.pid();
    match self.owner {
      | None => {
        self.owner = Some(pid);
        self.core.mark_attach();
        Ok(())
      },
      | Some(existing) if existing == pid => {
        // Re-attach by the same actor is permitted.
        self.core.mark_attach();
        Ok(())
      },
      | Some(_) => Err(SpawnError::DispatcherAlreadyOwned),
    }
  }

  fn unregister_actor(&mut self, actor: &ArcShared<ActorCell>) {
    let pid = actor.pid();
    if let Some(owner) = self.owner
      && owner == pid
    {
      self.owner = None;
    }
    self.core.mark_detach();
  }
}
