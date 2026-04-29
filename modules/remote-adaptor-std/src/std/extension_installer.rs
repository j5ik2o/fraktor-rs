//! Extension installer that wires [`crate::std::tcp_transport::TcpRemoteTransport`]
//! into `remote-core`'s `Remote` type.

#[cfg(test)]
mod tests;

mod remoting_extension_installer;

pub use remoting_extension_installer::RemotingExtensionInstaller;
