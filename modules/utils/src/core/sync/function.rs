#[cfg(target_has_atomic = "ptr")]
#[allow(dead_code)]
pub(crate) type SharedFnTarget<Args, Output> = dyn Fn(Args) -> Output + Send + Sync + 'static;

#[cfg(not(target_has_atomic = "ptr"))]
#[allow(dead_code)]
pub(crate) type SharedFnTarget<Args, Output> = dyn Fn(Args) -> Output + 'static;

/// Shared factory helpers.
mod shared_factory;
/// Shared function helpers.
mod shared_fn;

#[allow(unused_imports)]
pub(crate) use shared_factory::SharedFactory;
#[allow(unused_imports)]
pub(crate) use shared_fn::SharedFn;
