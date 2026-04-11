//! Workspace-wide default lock driver aliases.
//!
//! `SharedLock::new` and `SharedRwLock::new` materialize their backing
//! mutex through these aliases so that the entire workspace shares a
//! single canonical lock driver chosen at the `utils-core` boundary.
//!
//! In `no_std` builds (the design constraint of `utils-core`) the only
//! viable choice is [`SpinSyncMutex`] / [`SpinSyncRwLock`], so the aliases
//! resolve unconditionally. Std/tokio adapters that want to substitute
//! `std::sync::Mutex` or `parking_lot::Mutex` should layer their override on
//! top of `ActorLockProvider` (the actor system surface hook) rather than
//! redefining the alias here — that keeps the no_std surface stable and
//! confines the runtime cost of dispatching through `dyn ActorLockProvider`
//! to the few factories that need it.

use super::{SpinSyncMutex, SpinSyncRwLock};

/// Default mutex driver used by [`super::SharedLock::new`].
///
/// Resolves to [`SpinSyncMutex`]. The alias exists so that future
/// alternative drivers can be plugged in by re-defining it (or by adding
/// `default-lock-*` features) without rewriting call sites that already
/// went through `SharedLock::new`. At runtime the choice can still be
/// overridden per `ActorSystem` via `ActorLockProvider` for tests and
/// special workloads (DebugSpinSyncMutex, parking_lot, …).
pub type DefaultLockDriver<T> = SpinSyncMutex<T>;

/// Default rwlock driver used by [`super::SharedRwLock::new`].
///
/// Mirrors [`DefaultLockDriver`]: resolves to [`SpinSyncRwLock`] and is
/// overridable through the same `ActorLockProvider` mechanism.
pub type DefaultRwLockDriver<T> = SpinSyncRwLock<T>;
