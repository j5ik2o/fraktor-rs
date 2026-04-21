//! Guard abstractions for wrapping actor `receive` invocations.

mod invoke_guard_factory;
mod noop_invoke_guard;
mod noop_invoke_guard_factory;

pub use invoke_guard_factory::InvokeGuardFactory;
pub use noop_invoke_guard::NoopInvokeGuard;
pub use noop_invoke_guard_factory::NoopInvokeGuardFactory;

use crate::core::kernel::actor::error::ActorError;

/// Wraps actor `receive` invocations and can transform failures before they reach supervision.
pub trait InvokeGuard: Send + Sync {
  /// Executes the provided `receive` closure under the guard.
  ///
  /// # Errors
  ///
  /// Returns any `ActorError` produced by the wrapped closure or by the guard itself.
  fn wrap_receive(&self, call: &mut dyn FnMut() -> Result<(), ActorError>) -> Result<(), ActorError>;

  /// Convenience method for concrete guard types.
  ///
  /// # Errors
  ///
  /// Returns any `ActorError` produced by the wrapped closure or by the guard itself.
  fn wrap<F>(&self, f: F) -> Result<(), ActorError>
  where
    F: FnOnce() -> Result<(), ActorError>,
    Self: Sized, {
    let mut call = Some(f);
    self.wrap_receive(&mut || match call.take() {
      | Some(inner) => inner(),
      | None => Err(ActorError::fatal("invoke guard called closure more than once")),
    })
  }
}
