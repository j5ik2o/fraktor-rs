#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unknown_lints)]
#![deny(cfg_std_forbid)]
#![cfg_attr(not(test), no_std)]

//! Typed persistence effector APIs for fraktor actors.
//!
//! This crate connects typed actors with the persistence kernel while keeping
//! aggregate actors on the normal `Behavior<M>` DSL.

extern crate alloc;

mod backoff_config;
mod event_adapter;
mod event_rejected_error;
mod event_seq;
mod event_sourced_effector;
mod event_sourced_effector_config;
mod event_sourced_effector_message_adapter;
mod event_sourced_effector_signal;
mod event_sourced_effector_signal_auth;
mod event_sourced_signal;
mod internal;
mod persistence_id;
mod persistence_mode;
mod published_event;
mod recovery;
mod retention_criteria;
mod snapshot_adapter;
mod snapshot_criteria;
mod snapshot_selection_criteria;
mod state_sourced_effector;
mod state_sourced_effector_config;
mod state_sourced_effector_message_adapter;
mod state_sourced_effector_signal;
mod state_sourced_effector_signal_auth;

pub use backoff_config::BackoffConfig;
pub use event_adapter::EventAdapter;
pub use event_rejected_error::EventRejectedError;
pub use event_seq::EventSeq;
pub use event_sourced_effector::EventSourcedEffector;
pub use event_sourced_effector_config::EventSourcedEffectorConfig;
pub use event_sourced_effector_message_adapter::EventSourcedEffectorMessageAdapter;
pub use event_sourced_effector_signal::EventSourcedEffectorSignal;
pub use event_sourced_signal::EventSourcedSignal;
pub use persistence_id::PersistenceId;
pub use persistence_mode::PersistenceMode;
pub use published_event::PublishedEvent;
pub use recovery::Recovery;
pub use retention_criteria::RetentionCriteria;
pub use snapshot_adapter::SnapshotAdapter;
pub use snapshot_criteria::SnapshotCriteria;
pub use snapshot_selection_criteria::SnapshotSelectionCriteria;
pub use state_sourced_effector::StateSourcedEffector;
pub use state_sourced_effector_config::StateSourcedEffectorConfig;
pub use state_sourced_effector_message_adapter::StateSourcedEffectorMessageAdapter;
pub use state_sourced_effector_signal::StateSourcedEffectorSignal;
