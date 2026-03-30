//! Internal IO implementation namespace.

#[cfg(feature = "compression")]
mod compression;

#[cfg(feature = "compression")]
pub use compression::Compression;
