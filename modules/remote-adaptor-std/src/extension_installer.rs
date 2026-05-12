//! Extension installer that wires [`crate::transport::tcp::TcpRemoteTransport`]
//! into `remote-core`'s `Remote` type.

#[cfg(test)]
#[path = "extension_installer_test.rs"]
mod tests;

mod remoting_extension_installer;

pub use remoting_extension_installer::RemotingExtensionInstaller;
