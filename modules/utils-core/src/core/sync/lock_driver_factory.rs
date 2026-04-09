use super::{lock_driver::LockDriver, rwlock_driver::RwLockDriver};

/// Factory seam for selecting a mutex driver family.
pub trait LockDriverFactory {
  /// Driver type produced for `T`.
  type Driver<T>: LockDriver<T>;
}

/// Factory seam for selecting an rwlock driver family.
pub trait RwLockDriverFactory {
  /// Driver type produced for `T`.
  type Driver<T>: RwLockDriver<T>;
}
