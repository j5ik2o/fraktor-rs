#[cfg(target_has_atomic = "ptr")]
pub(super) type SharedFnTarget<Args, Output> = dyn Fn(Args) -> Output + Send + Sync + 'static;

#[cfg(not(target_has_atomic = "ptr"))]
pub(super) type SharedFnTarget<Args, Output> = dyn Fn(Args) -> Output + 'static;

/// Shared factory helpers.
mod shared_factory;
/// Shared function helpers.
mod shared_fn;

pub use shared_factory::SharedFactory;
pub use shared_fn::SharedFn;
