//! Dispatcher registry — placeholder for future implementation.
//!
//! This module corresponds to `org.apache.pekko.actor.typed.Dispatchers` in the
//! Pekko reference implementation and reserves the `typed::dispatchers` package
//! boundary.
//!
//! # Pekko equivalent
//!
//! ```scala
//! abstract class Dispatchers {
//!   def lookup(selector: DispatcherSelector): ExecutionContextExecutor
//!   def shutdown(): Unit
//! }
//! ```
//!
//! When implemented, this module should expose a `Dispatchers` type that resolves
//! a thread-pool executor from a [`crate::core::typed::DispatcherSelector`].
//!
//! # Do not remove
//!
//! This file is an intentional placeholder. It must not be deleted even though
//! it contains no types yet. Removing it would lose the `typed::dispatchers`
//! package boundary that future dispatcher-resolution work depends on.
