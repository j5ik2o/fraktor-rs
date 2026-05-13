#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unknown_lints)]
#![deny(cfg_std_forbid)]
#![cfg_attr(not(test), no_std)]

//! Typed persistence effector API for fraktor actors.
//!
//! This crate connects typed actors with the persistence kernel while keeping
//! aggregate actors on the normal `Behavior<M>` DSL.

extern crate alloc;

mod backoff_config;
mod internal;
mod persistence_effector;
mod persistence_effector_config;
mod persistence_effector_message_adapter;
mod persistence_effector_signal;
mod persistence_id;
mod persistence_mode;
mod retention_criteria;
mod snapshot_criteria;

pub use backoff_config::BackoffConfig;
pub use persistence_effector::PersistenceEffector;
pub use persistence_effector_config::PersistenceEffectorConfig;
pub use persistence_effector_message_adapter::PersistenceEffectorMessageAdapter;
pub use persistence_effector_signal::PersistenceEffectorSignal;
pub use persistence_id::PersistenceId;
pub use persistence_mode::PersistenceMode;
pub use retention_criteria::RetentionCriteria;
pub use snapshot_criteria::SnapshotCriteria;
