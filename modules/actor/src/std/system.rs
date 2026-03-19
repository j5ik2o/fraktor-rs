//! Actor system bindings for the standard toolbox.

// NOTE: CoordinatedShutdownPhase と CoordinatedShutdownReason は core::system に移設済み。
// no-parent-reexport lint により std からの re-export は禁止されているため、
// 利用者は crate::core::system::{CoordinatedShutdownPhase, CoordinatedShutdownReason}
// を直接参照すること。

mod base;
#[cfg(feature = "tokio-executor")]
mod coordinated_shutdown;
#[cfg(feature = "tokio-executor")]
mod coordinated_shutdown_error;
#[cfg(feature = "tokio-executor")]
mod coordinated_shutdown_id;
#[cfg(feature = "tokio-executor")]
mod coordinated_shutdown_installer;

pub use base::*;
#[cfg(feature = "tokio-executor")]
pub use coordinated_shutdown::*;
#[cfg(feature = "tokio-executor")]
pub use coordinated_shutdown_error::*;
#[cfg(feature = "tokio-executor")]
pub use coordinated_shutdown_id::*;
#[cfg(feature = "tokio-executor")]
pub use coordinated_shutdown_installer::*;
