//! Utility package for kernel-level helpers.
//!
//! Corresponds to `akka.util` / `org.apache.pekko.util` in Pekko.

mod byte_string;
pub mod futures;

pub use byte_string::ByteString;
