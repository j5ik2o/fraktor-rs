use super::lock_driver::LockDriver;

/// Factory seam for selecting a mutex driver family.
pub trait LockDriverFactory {
  /// Driver type produced for `T`.
  type Driver<T>: LockDriver<T>;
}
