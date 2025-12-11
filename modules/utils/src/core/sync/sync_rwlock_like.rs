use core::ops::{Deref, DerefMut};

mod spin_sync_rwlock;

pub use spin_sync_rwlock::SpinSyncRwLock;

/// Generic read-write lock abstraction for runtime-agnostic code.
pub trait SyncRwLockLike<T> {
  /// Guard type returned by [`SyncRwLockLike::read`].
  type ReadGuard<'a>: Deref<Target = T>
  where
    Self: 'a,
    T: 'a;

  /// Guard type returned by [`SyncRwLockLike::write`].
  type WriteGuard<'a>: DerefMut<Target = T>
  where
    Self: 'a,
    T: 'a;

  /// Creates a new read-write lock wrapping the provided value.
  fn new(value: T) -> Self;

  /// Consumes the lock and returns the inner value.
  fn into_inner(self) -> T;

  /// Acquires a shared read guard.
  fn read(&self) -> Self::ReadGuard<'_>;

  /// Acquires an exclusive write guard.
  fn write(&self) -> Self::WriteGuard<'_>;
}

/// Convenience alias for guards produced by [`SyncRwLockLike::read`].
pub type SyncRwLockReadGuard<'a, L, T> = <L as SyncRwLockLike<T>>::ReadGuard<'a>;
/// Convenience alias for guards produced by [`SyncRwLockLike::write`].
pub type SyncRwLockWriteGuard<'a, L, T> = <L as SyncRwLockLike<T>>::WriteGuard<'a>;
