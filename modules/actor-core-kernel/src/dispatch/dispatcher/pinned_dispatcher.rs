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
  dispatcher_config::DispatcherConfig, dispatcher_core::DispatcherCore, executor_shared::ExecutorShared,
  message_dispatcher::MessageDispatcher,
};
use crate::actor::{ActorCell, Pid, spawn::SpawnError};

/// Dispatcher dedicated to a single actor.
///
/// Construction normalises throughput to `usize::MAX` and clears the throughput
/// deadline regardless of the supplied [`DispatcherConfig`], matching Pekko's
/// behaviour for `PinnedDispatcher`.
///
/// # 1 actor / 1 thread exclusion (AC-M1)
///
/// Mirrors Pekko `dispatch/PinnedDispatcher.scala:44-59`:
///
/// - Pekko: `@volatile var owner: ActorCell`, assigned inside `register` after an `if ((actor ne
///   null) && actorCell != actor) throw`.
/// - fraktor-rs: [`Self::owner`] holds `Option<Pid>`, and [`Self::register_actor`] returns
///   [`SpawnError::DispatcherAlreadyOwned`] for the equivalent conflict case (see its rustdoc for
///   the branch-by-branch correspondence).
///
/// Pekko relies on the external lock held by `MessageDispatcher.attach`;
/// fraktor-rs achieves the equivalent serialisation through `&mut self`
/// on the dispatcher trait plus the mutex inside
/// [`MessageDispatcherShared`], so races between concurrent
/// `register_actor` / `unregister_actor` invocations are impossible
/// without additional atomics (no `AtomicCell<Option<Pid>>` needed).
pub struct PinnedDispatcher {
  core:  DispatcherCore,
  owner: Option<Pid>,
}

impl PinnedDispatcher {
  /// Constructs a new `PinnedDispatcher` from the supplied settings and executor.
  ///
  /// The settings are normalised to `throughput = usize::MAX`,
  /// `throughput_deadline = None` before being handed to [`DispatcherCore`].
  #[must_use]
  pub fn new(settings: &DispatcherConfig, executor: ExecutorShared) -> Self {
    // SAFETY: `usize::MAX` is non-zero on every supported target.
    let max_throughput = unsafe { NonZeroUsize::new_unchecked(usize::MAX) };
    let normalised = settings.clone().with_throughput(max_throughput).with_throughput_deadline(None);
    Self { core: DispatcherCore::new(&normalised, executor), owner: None }
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

  /// Registers an actor on this dispatcher, enforcing the
  /// 1 actor / 1 thread exclusion contract.
  ///
  /// # Pekko parity (`PinnedDispatcher.scala:48-54`)
  ///
  /// | Pekko branch | fraktor-rs branch |
  /// |-------------|-------------------|
  /// | `actor eq null` (owner unset) | `None` → assign `owner = Some(pid)` |
  /// | `actor ne null && actorCell eq actor` (same re-attach) | `Some(existing) if existing == pid` → idempotent accept |
  /// | `actor ne null && actorCell ne actor` (conflict) | `Some(_)` → `Err(SpawnError::DispatcherAlreadyOwned)` |
  ///
  /// Pekko throws `IllegalArgumentException` for the conflict case; the
  /// fraktor-rs translation is a recoverable `Err` so the caller can react
  /// without panicking. fraktor-rs skips the unconditional re-assignment
  /// that Pekko performs in the `same-actor` case (same value assignment
  /// would be a no-op) and relies on `mark_attach` to keep the inhabitant
  /// counter monotonic across repeated attaches.
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

  /// Unregisters an actor, clearing the owner slot when it matches.
  ///
  /// # Pekko parity (`PinnedDispatcher.scala:56-59`)
  ///
  /// Pekko unconditionally sets `owner = null` after delegating to
  /// `super.unregister(actor)` because its callers already enforce
  /// "only the current owner can call unregister". fraktor-rs adds a
  /// defensive `owner == pid` check: if an unrelated pid is passed,
  /// the owner slot stays intact and only `mark_detach` runs. The
  /// difference has no observable effect under correct use, and
  /// guards against future callers that might misuse the API.
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
