//! Public termination observation contract.

use core::{future::IntoFuture, task::Waker};

use fraktor_utils_core_rs::sync::ArcShared;

use super::{blocker::Blocker, termination_future::TerminationFuture, termination_state::TerminationState};

/// Public contract for observing actor system termination.
///
/// `TerminationSignal` is clone-able and non-consuming: any number of clones
/// can independently observe the same termination event without interfering
/// with each other.
///
/// # Async usage
///
/// ```no_run
/// # fn example(system: fraktor_actor_core_kernel_rs::system::ActorSystem) {
/// let signal = system.when_terminated();
/// // signal.await; // resolves when the system terminates
/// # }
/// ```
///
/// # Sync usage (requires std adapter `Blocker`)
///
/// ```no_run
/// # fn example(signal: fraktor_actor_core_kernel_rs::system::TerminationSignal, blocker: &dyn fraktor_actor_core_kernel_rs::system::Blocker) {
/// signal.wait_blocking(blocker);
/// # }
/// ```
#[derive(Clone)]
pub struct TerminationSignal {
  state: ArcShared<TerminationState>,
}

impl TerminationSignal {
  /// Creates a signal backed by the given shared termination state.
  #[must_use]
  pub(crate) const fn new(state: ArcShared<TerminationState>) -> Self {
    Self { state }
  }

  /// Creates a signal that reports the system as already terminated.
  ///
  /// Used when the backing system state has been deallocated.
  #[must_use]
  pub fn already_terminated() -> Self {
    let state = TerminationState::new();
    state.mark_terminated();
    Self { state: ArcShared::new(state) }
  }

  /// Returns `true` once the actor system has fully terminated.
  ///
  /// This check is monotonic: once it returns `true`, it will never return
  /// `false` again.
  #[must_use]
  pub fn is_terminated(&self) -> bool {
    self.state.is_terminated()
  }

  /// Blocks the current thread until termination completes, using the
  /// provided [`Blocker`] implementation.
  pub fn wait_blocking(&self, blocker: &dyn Blocker) {
    blocker.block_until(&|| self.is_terminated());
  }

  /// Registers a waker to be notified on termination (used by the
  /// [`TerminationFuture`] implementation).
  pub(crate) fn register_waker(&self, waker: &Waker) {
    self.state.register_waker(waker);
  }
}

impl IntoFuture for TerminationSignal {
  type IntoFuture = TerminationFuture;
  type Output = ();

  fn into_future(self) -> Self::IntoFuture {
    TerminationFuture::new(self)
  }
}
