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
#![deny(unreachable_pub)]
#![allow(unknown_lints)]
#![deny(cfg_std_forbid)]
#![cfg_attr(not(test), no_std)]

//! Typed interface wrappers around the untyped runtime.

extern crate alloc;

/// Typed actor primitives (actors, contexts, references).
pub mod actor;
/// Typed actor reference wrapper promoted to the typed root.
mod actor_ref;
/// Actor-ref serialization extension for typed APIs.
mod actor_ref_resolver;
/// Setup wrapper for replacing the default actor-ref resolver extension.
mod actor_ref_resolver_setup;
/// Typed metadata tags facade for actor props.
mod actor_tags;
/// Typed backoff supervision strategy facade.
mod backoff_supervisor_strategy;
/// Typed behavior representation.
mod behavior;
/// Cross-cutting concern interceptor for typed behaviors.
mod behavior_interceptor;
/// Point-to-point reliable delivery between a producer and consumer actor.
pub mod delivery;
/// Dispatcher selection strategy for typed props.
mod dispatcher_selector;
/// Typed dispatcher lookup facade.
mod dispatchers;
/// DSL package for typed actor development (Behaviors, stash, timers, ask patterns).
pub mod dsl;
/// Typed event stream package for subscribing and publishing to the system event stream.
pub mod eventstream;
/// Public extension point for custom typed behavior implementations.
mod extensible_behavior;
/// Generic setup wrapper for configuring extensions during system bootstrap.
mod extension_setup;
/// Internal implementation types (BehaviorRunner, TypedActorAdapter, scheduler internals).
mod internal;
/// Logging options for typed behavior helpers.
mod log_options;
/// Mailbox selection strategy for typed props.
mod mailbox_selector;
/// Message adapter primitives bridging external protocols.
pub mod message_adapter;
/// Messages and signals delivered to typed behaviors (Pekko MessageAndSignals).
pub mod message_and_signals;
/// Typed props that wrap untyped props.
mod props;
/// Typed pub/sub package for topic actors and commands.
pub mod pubsub;
/// Typed receptionist package for service discovery primitives.
pub mod receptionist;
/// Common recipient abstraction for typed and untyped actor references.
mod recipient_ref;
/// Typed restart supervision strategy facade.
mod restart_supervisor_strategy;
/// Typed scheduler facade.
mod scheduler;
/// Pekko-inspired spawn protocol for typed actors.
mod spawn_protocol;
/// Typed supervisor strategy factory facade.
mod supervisor_strategy;
/// Typed actor system interface.
mod system;
/// Immutable metadata snapshot for typed actor systems.
mod typed_actor_system_config;
/// System-level log handle for typed actor systems.
mod typed_actor_system_log;
pub use actor_ref::TypedActorRef;
pub use actor_ref_resolver::ActorRefResolver;
pub use actor_ref_resolver_setup::ActorRefResolverSetup;
pub use actor_tags::ActorTags;
pub use backoff_supervisor_strategy::BackoffSupervisorStrategy;
pub use behavior::Behavior;
pub use behavior_interceptor::BehaviorInterceptor;
pub use dispatcher_selector::DispatcherSelector;
pub use dispatchers::Dispatchers;
pub use extensible_behavior::ExtensibleBehavior;
pub use extension_setup::ExtensionSetup;
pub use log_options::LogOptions;
pub use mailbox_selector::MailboxSelector;
pub use props::TypedProps;
pub use recipient_ref::RecipientRef;
pub use restart_supervisor_strategy::RestartSupervisorStrategy;
pub use scheduler::Scheduler;
pub use spawn_protocol::SpawnProtocol;
pub use supervisor_strategy::SupervisorStrategy;
pub use system::TypedActorSystem;
pub use typed_actor_system_config::TypedActorSystemConfig;
pub use typed_actor_system_log::TypedActorSystemLog;
mod test_support;
#[cfg(test)]
mod tests;
