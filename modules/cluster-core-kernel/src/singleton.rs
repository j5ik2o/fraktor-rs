//! Cluster Singleton configuration, validation, and error vocabulary.

mod cluster_singleton_config_error;
mod cluster_singleton_manager;
mod cluster_singleton_manager_config;
mod cluster_singleton_manager_effect;
mod cluster_singleton_manager_message;
mod cluster_singleton_manager_outcome;
mod cluster_singleton_manager_phase;
mod cluster_singleton_proxy;
mod cluster_singleton_proxy_config;
mod lease_usage_config;
mod singleton_stuck_phase;

pub use cluster_singleton_config_error::ClusterSingletonConfigError;
pub use cluster_singleton_manager::{ClusterSingletonManager, is_older_member};
pub use cluster_singleton_manager_config::ClusterSingletonManagerConfig;
pub use cluster_singleton_manager_effect::ClusterSingletonManagerEffect;
pub use cluster_singleton_manager_message::ClusterSingletonManagerMessage;
pub use cluster_singleton_manager_outcome::ClusterSingletonManagerOutcome;
pub use cluster_singleton_manager_phase::ClusterSingletonManagerPhase;
pub use cluster_singleton_proxy::{ClusterSingletonProxy, ClusterSingletonProxyEffect, ClusterSingletonProxyOutcome};
pub use cluster_singleton_proxy_config::ClusterSingletonProxyConfig;
pub use lease_usage_config::LeaseUsageConfig;
pub use singleton_stuck_phase::SingletonStuckPhase;
