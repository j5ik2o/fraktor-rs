//! Actor reference providers for remote communication.
//!
//! This module provides implementations of [`ActorRefProvider`] for different
//! transport types (Loopback, Remote TCP, Tokio TCP) and their installers.

mod loopback;
mod loopback_installer;
mod loopback_router;
mod loopback_serialization_setup;
mod remote;
mod remote_error;
mod remote_installer;
mod tokio;
mod tokio_installer;

pub use loopback::{LoopbackActorRefProvider, LoopbackActorRefProviderGeneric};
pub use loopback_installer::LoopbackActorRefProviderInstaller;
pub(crate) use loopback_router::unregister_endpoint;
pub use loopback_serialization_setup::default_loopback_setup;
pub use remote::{RemoteActorRefProvider, RemoteActorRefProviderGeneric};
pub use remote_error::RemoteActorRefProviderError;
pub use remote_installer::RemoteActorRefProviderInstaller;
pub use tokio::{TokioActorRefProvider, TokioActorRefProviderGeneric};
pub use tokio_installer::TokioActorRefProviderInstaller;
