//! Extension installer that wires the adapter components into a single
//! `StdRemoting` aggregate implementing
//! [`fraktor_remote_core_rs::core::extension::Remoting`].
//!
//! `StdRemoting` is the Phase B counterpart to the legacy
//! `RemotingControlHandle` god object: it owns the
//! [`crate::std::tcp_transport::TcpRemoteTransport`],
//! [`crate::std::association_runtime::AssociationRegistry`], the
//! [`crate::std::watcher_actor::WatcherActorHandle`], and the
//! [`crate::std::provider::StdRemoteActorRefProvider`], and exposes the same
//! `Remoting` lifecycle surface as the pure core trait.
//!
//! The actor-core `ExtensionInstaller` registration sits next to the
//! aggregate, mirroring the legacy
//! `modules/remote/src/std/remoting_extension_installer.rs` shape, so the
//! existing installer call sites in `cluster-adaptor-std` etc. (Phase D
//! switchover) can adopt it with minimal churn.

#[cfg(test)]
mod tests;

mod base;
mod remoting_extension_installer;

pub use base::StdRemoting;
pub use remoting_extension_installer::RemotingExtensionInstaller;
