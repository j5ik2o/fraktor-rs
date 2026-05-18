//! Extension installer that wires [`crate::transport::tcp::TcpRemoteTransport`]
//! into `remote-core`'s `Remote` type.

#[cfg(test)]
#[path = "extension_installer_test.rs"]
mod tests;

mod flush_gate;
mod remoting_extension_installer;

pub(crate) use flush_gate::{StdFlushGate, StdFlushNotification};
pub(crate) use remoting_extension_installer::RemoteProviderFlushHandles;
pub use remoting_extension_installer::RemotingExtensionInstaller;
