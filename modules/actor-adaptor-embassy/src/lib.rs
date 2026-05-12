#![no_std]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unknown_lints)]

//! Embassy adaptors for fraktor actor systems.
//!
//! This crate keeps Embassy-specific task, signal, and timer integration out of
//! `actor-core-kernel`. It provides an actor-system configuration helper,
//! dispatcher bindings, a monotonic mailbox clock, and [`EmbassyTickDriver`].
//!
//! The dispatcher adapter is driven by an Embassy task through
//! [`EmbassyExecutorDriver`](dispatch::EmbassyExecutorDriver);
//! [`Executor::execute`](fraktor_actor_core_kernel_rs::dispatch::dispatcher::Executor::execute)
//! only enqueues ready work and wakes that task. Queue saturation is reported as
//! an executor submit error instead of blocking.
//!
//! [`EmbassyTickDriver`] must be constructed with an [`embassy_executor::SendSpawner`]
//! before provisioning can start scheduler ticks. The default value is useful for
//! inspecting configuration shape, but provisioning it returns handle-unavailable.
//!
//! Remote, stream, persistence, networking, and storage adaptors remain outside
//! this crate. Applications should combine this crate with domain-specific
//! adapters when those capabilities are available for their target.

extern crate alloc;

mod actor;
/// Dispatcher executor bindings for Embassy.
pub mod dispatch;
mod tick_driver;
mod time;

pub use actor::embassy_actor_system_config;
pub use tick_driver::EmbassyTickDriver;
pub use time::embassy_monotonic_mailbox_clock;
