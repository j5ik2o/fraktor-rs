//! Port contract for synchronous blocking wait.

/// Port contract for blocking the current thread until a condition is met.
///
/// Implementations live in platform-specific adapter crates (e.g.
/// `fraktor-actor-adaptor-rs` provides a `Condvar`-based `StdBlocker`).
/// Core code references only this trait, keeping `std` dependencies out of
/// the `core` module.
pub trait Blocker: Send + Sync {
  /// Blocks the current thread until `condition` returns `true`.
  ///
  /// Platform-appropriate sleeping or parking mechanisms are preferred
  /// over busy-wait spin loops. A spin-based fallback
  /// ([`SpinBlocker`](super::SpinBlocker)) exists for no_std and test
  /// environments where parking is unavailable.
  fn block_until(&self, condition: &dyn Fn() -> bool);
}
