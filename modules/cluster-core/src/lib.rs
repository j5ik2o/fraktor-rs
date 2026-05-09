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

//! Cluster runtime components compatible with Proto.Actor/Pekko semantics.

extern crate alloc;

mod block_list_provider;
mod cluster_api;
mod cluster_api_error;
mod cluster_core;
mod cluster_error;
mod cluster_event;
mod cluster_event_type;
mod cluster_extension;
mod cluster_extension_config;
mod cluster_extension_id;
mod cluster_extension_installer;
mod cluster_metrics;
mod cluster_metrics_snapshot;
/// Cluster provider implementations and traits.
pub mod cluster_provider;
mod cluster_provider_error;
mod cluster_provider_shared;
mod cluster_request_error;
mod cluster_resolve_error;
mod cluster_router_group;
mod cluster_router_group_config;
mod cluster_router_pool;
mod cluster_router_pool_config;
mod cluster_subscription_initial_state_mode;
mod cluster_topology;
mod config_validation;
/// Downing strategy abstractions and default implementations.
pub mod downing_provider;
/// Failure detector traits and registry used by the cluster layer.
pub mod failure_detector;
/// Virtual actor (grain) API, RPC routing, and codec abstraction.
pub mod grain;
/// PID resolution, identity lookup, and rendezvous hashing.
pub mod identity;
mod join_config_compat_checker;
/// Membership management, gossip dissemination, and node lifecycle.
pub mod membership;
mod metrics_error;
/// Outbound message pipeline coordination.
pub mod outbound;
/// Actor placement coordination and activation lifecycle.
pub mod placement;
/// Cluster-wide publish/subscribe messaging coordination.
pub mod pub_sub;
mod startup_mode;
mod topology_apply_error;
mod topology_update;

pub use block_list_provider::BlockListProvider;
pub use cluster_api::ClusterApi;
pub use cluster_api_error::ClusterApiError;
pub use cluster_core::ClusterCore;
pub use cluster_error::ClusterError;
pub use cluster_event::ClusterEvent;
pub use cluster_event_type::ClusterEventType;
pub use cluster_extension::ClusterExtension;
pub use cluster_extension_config::ClusterExtensionConfig;
pub use cluster_extension_id::ClusterExtensionId;
pub use cluster_extension_installer::{ClusterExtensionInstaller, ClusterProviderFactory};
pub use cluster_metrics::ClusterMetrics;
pub use cluster_metrics_snapshot::ClusterMetricsSnapshot;
pub use cluster_provider_error::ClusterProviderError;
pub use cluster_provider_shared::ClusterProviderShared;
pub use cluster_request_error::ClusterRequestError;
pub use cluster_resolve_error::ClusterResolveError;
pub use cluster_router_group::ClusterRouterGroup;
pub use cluster_router_group_config::ClusterRouterGroupConfig;
pub use cluster_router_pool::ClusterRouterPool;
pub use cluster_router_pool_config::ClusterRouterPoolConfig;
pub use cluster_subscription_initial_state_mode::ClusterSubscriptionInitialStateMode;
pub use cluster_topology::ClusterTopology;
pub use config_validation::ConfigValidation;
pub use join_config_compat_checker::JoinConfigCompatChecker;
pub use metrics_error::MetricsError;
pub use startup_mode::StartupMode;
pub use topology_apply_error::TopologyApplyError;
pub use topology_update::TopologyUpdate;
