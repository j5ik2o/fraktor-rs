#[allow(clippy::disallowed_types)]
mod arc_shared;
/// Async-aware mutex abstractions shared across runtimes.
pub mod async_mutex_like;
#[allow(clippy::disallowed_types)]
mod flag;
/// Helper traits for shared function and factory closures.
pub mod function;
/// Policies for detecting interrupt contexts prior to blocking operations.
pub mod interrupt;
#[cfg(feature = "alloc")]
#[allow(clippy::disallowed_types)]
mod rc_shared;
mod shared;
mod state;
mod static_ref_shared;
/// Synchronous mutex abstractions shared across runtimes.
pub mod sync_mutex_like;

pub use arc_shared::ArcShared;
pub use flag::Flag;
pub use function::{SharedFactory, SharedFn};
pub use interrupt::{CriticalSectionInterruptPolicy, InterruptContextPolicy, NeverInterruptPolicy};
#[cfg(feature = "alloc")]
pub use rc_shared::RcShared;
pub use shared::{SendBound, Shared, SharedBound, SharedDyn};
pub use state::StateCell;
pub use static_ref_shared::StaticRefShared;
