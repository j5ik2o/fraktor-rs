use super::rwlock_driver::RwLockDriver;

/// Factory seam for selecting an rwlock driver family.
pub trait RwLockDriverFactory {
  /// Driver type produced for `T`.
  type Driver<T>: RwLockDriver<T>;
}
