#![deny(missing_docs)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_types, clippy::redundant_clone))]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::missing_errors_doc)]
#![deny(clippy::missing_panics_doc)]
#![deny(clippy::missing_safety_doc)]
#![cfg_attr(not(test), deny(clippy::redundant_clone))]
#![deny(clippy::redundant_field_names)]
#![deny(clippy::redundant_pattern)]
#![deny(clippy::redundant_static_lifetimes)]
#![deny(clippy::unnecessary_to_owned)]
#![deny(clippy::unnecessary_struct_initialization)]
#![deny(clippy::needless_borrow)]
#![deny(clippy::needless_pass_by_value)]
#![deny(clippy::manual_ok_or)]
#![deny(clippy::manual_map)]
#![deny(clippy::manual_let_else)]
#![deny(clippy::manual_strip)]
#![deny(clippy::unused_async)]
#![deny(clippy::unused_self)]
#![deny(clippy::unnecessary_wraps)]
#![deny(clippy::unreachable)]
#![deny(clippy::empty_enums)]
#![deny(clippy::no_effect)]
#![deny(dropping_copy_types)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), deny(clippy::expect_used))]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::print_stdout)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::missing_const_for_fn)]
#![deny(clippy::must_use_candidate)]
#![deny(clippy::trivially_copy_pass_by_ref)]
#![deny(clippy::clone_on_copy)]
#![deny(clippy::len_without_is_empty)]
#![deny(clippy::wrong_self_convention)]
#![deny(clippy::from_over_into)]
#![deny(clippy::eq_op)]
#![deny(clippy::bool_comparison)]
#![deny(clippy::needless_bool)]
#![deny(clippy::match_like_matches_macro)]
#![deny(clippy::manual_assert)]
#![deny(clippy::naive_bytecount)]
#![deny(clippy::if_same_then_else)]
#![deny(clippy::cmp_null)]
#![deny(unreachable_pub)]
#![allow(unknown_lints)]
#![deny(cfg_std_forbid)]
#![cfg_attr(not(test), no_std)]

//! Persistence support for the fraktor actor runtime.
//!
//! This crate provides the core persistence abstractions for event sourcing:
//!
//! - [`Journal`] - Event journal trait with GATs pattern
//! - [`SnapshotStore`] - Snapshot store trait with GATs pattern
//! - [`PersistentActor`] - Persistent actor trait (Pekko-compatible)
//! - [`PersistentRepr`] - Persistent event representation
//! - [`InMemoryJournal`] - In-memory journal for testing
//! - [`InMemorySnapshotStore`] - In-memory snapshot store for testing
//! - [`PersistenceExtension`] - Extension for ActorSystem integration
//!
//! Use `fraktor_persistence_core_rs` for convenient imports.

extern crate alloc;

mod at_least_once_delivery;
mod at_least_once_delivery_config;
mod at_least_once_delivery_snapshot;
mod durable_state_error;
mod durable_state_store;
mod durable_state_store_provider;
mod durable_state_store_registry;
mod durable_state_update_store;
mod eventsourced;

mod event_adapters;
mod event_seq;
mod identity_event_adapter;
mod in_memory_journal;
mod in_memory_snapshot_store;
mod journal;
mod journal_actor;
mod journal_actor_config;
mod journal_error;
mod journal_message;
mod journal_response;
mod journal_response_action;
mod pending_handler_invocation;
mod persistence_context;
mod persistence_error;
mod persistence_extension;
mod persistence_extension_id;
mod persistence_extension_installer;
mod persistence_extension_shared;
mod persistence_plugin_proxy;
mod persistent_actor;
mod persistent_actor_adapter;
mod persistent_actor_state;
mod persistent_envelope;
mod persistent_fsm;
mod persistent_props;
mod persistent_repr;
mod read_event_adapter;
mod recovery;
mod recovery_timed_out;
mod redelivery_tick;
mod snapshot;
mod snapshot_actor;
mod snapshot_actor_config;
mod snapshot_error;
mod snapshot_message;
mod snapshot_metadata;
mod snapshot_response;
mod snapshot_response_action;
mod snapshot_selection_criteria;
mod snapshot_store;
mod stash_overflow_strategy;
mod tagged;
mod unconfirmed_delivery;
mod unconfirmed_warning;
mod write_event_adapter;

pub use at_least_once_delivery::AtLeastOnceDelivery;
pub use at_least_once_delivery_config::AtLeastOnceDeliveryConfig;
pub use at_least_once_delivery_snapshot::AtLeastOnceDeliverySnapshot;
pub use durable_state_error::DurableStateError;
pub use durable_state_store::DurableStateStore;
pub use durable_state_store_provider::DurableStateStoreProvider;
pub use durable_state_store_registry::DurableStateStoreRegistry;
pub use durable_state_update_store::DurableStateUpdateStore;
pub use event_adapters::EventAdapters;
pub use event_seq::EventSeq;
pub use eventsourced::Eventsourced;
pub use identity_event_adapter::IdentityEventAdapter;
pub use in_memory_journal::InMemoryJournal;
pub use in_memory_snapshot_store::InMemorySnapshotStore;
pub use journal::Journal;
pub use journal_actor::JournalActor;
pub use journal_actor_config::JournalActorConfig;
pub use journal_error::JournalError;
pub use journal_message::JournalMessage;
pub use journal_response::JournalResponse;
pub use pending_handler_invocation::PendingHandlerInvocation;
pub use persistence_context::PersistenceContext;
pub use persistence_error::PersistenceError;
pub use persistence_extension::PersistenceExtension;
pub use persistence_extension_id::PersistenceExtensionId;
pub use persistence_extension_installer::PersistenceExtensionInstaller;
pub use persistence_extension_shared::PersistenceExtensionShared;
pub use persistence_plugin_proxy::PersistencePluginProxy;
pub use persistent_actor::PersistentActor;
pub use persistent_actor_state::PersistentActorState;
pub use persistent_envelope::PersistentEnvelope;
pub use persistent_fsm::PersistentFsm;
pub use persistent_props::{persistent_props, spawn_persistent};
pub use persistent_repr::PersistentRepr;
pub use read_event_adapter::ReadEventAdapter;
pub use recovery::Recovery;
pub use recovery_timed_out::RecoveryTimedOut;
pub use redelivery_tick::RedeliveryTick;
pub use snapshot::Snapshot;
pub use snapshot_actor::SnapshotActor;
pub use snapshot_actor_config::SnapshotActorConfig;
pub use snapshot_error::SnapshotError;
pub use snapshot_message::SnapshotMessage;
pub use snapshot_metadata::SnapshotMetadata;
pub use snapshot_response::SnapshotResponse;
pub use snapshot_selection_criteria::SnapshotSelectionCriteria;
pub use snapshot_store::SnapshotStore;
pub use stash_overflow_strategy::StashOverflowStrategy;
pub use tagged::Tagged;
pub use unconfirmed_delivery::UnconfirmedDelivery;
pub use unconfirmed_warning::UnconfirmedWarning;
pub use write_event_adapter::WriteEventAdapter;
