#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout, clippy::dbg_macro)]
#![deny(clippy::missing_errors_doc, clippy::missing_panics_doc)]

//! Standard library helpers for Cellactor runtime integrations.

/// Actor primitives specialised for the standard toolbox.
pub mod actor_prim;

/// Messaging primitives specialised for the standard toolbox.
pub mod messaging;

/// Props and dispatcher configuration bindings for the standard toolbox.
pub mod props;

/// Mailbox bindings for the standard toolbox.
pub mod mailbox;

/// Actor system bindings for the standard toolbox.
pub mod system;

/// Event stream bindings for the standard toolbox.
pub mod eventstream;

/// Deadletter bindings for the standard toolbox.
pub mod deadletter;

/// Future utilities specialised for the standard toolbox.
pub mod futures;

/// Error utilities specialised for the standard toolbox.
pub mod error;
