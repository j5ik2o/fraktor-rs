//! Actor-specific helpers that require the standard library.

mod panic_invoke_guard;
mod panic_invoke_guard_factory;

use fraktor_actor_core_rs::core::kernel::actor::setup::ActorSystemConfig;
pub use panic_invoke_guard::PanicInvokeGuard;
pub use panic_invoke_guard_factory::PanicInvokeGuardFactory;

/// Installs the std panic guard into an actor-system configuration.
#[must_use]
pub fn install_panic_invoke_guard(config: ActorSystemConfig) -> ActorSystemConfig {
  config.with_invoke_guard_factory(PanicInvokeGuardFactory::shared())
}
