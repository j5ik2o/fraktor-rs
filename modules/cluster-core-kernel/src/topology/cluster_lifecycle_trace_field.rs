//! Cluster lifecycle trace field name contract (single authoritative definition).
//!
//! This module defines `pub const` values for structured trace field names
//! used when observing cluster lifecycle transitions. All cluster lifecycle
//! tracing output in the std layer MUST reference these constants instead of
//! ad-hoc string literals (requirement 4.3, 4.4).
//!
//! # Transition kinds
//!
//! Each `TRANSITION_*` constant identifies a distinct lifecycle transition kind.
//! Values are unique across all constants (requirement 4.1).
//!
//! # Field names
//!
//! `FIELD_*` constants name the structured fields that accompany lifecycle
//! trace events, covering member identification and data-center identity
//! (requirement 4.2).

#[cfg(test)]
#[path = "cluster_lifecycle_trace_field_test.rs"]
mod tests;

// --- 遷移種別定数（要件 4.1: 遷移種別ごとに一意な値） ---

/// Trace field value identifying a member-join transition.
pub const TRANSITION_JOIN: &str = "join";

/// Trace field value identifying a member-up transition.
pub const TRANSITION_UP: &str = "up";

/// Trace field value identifying a member-leave transition.
pub const TRANSITION_LEAVE: &str = "leave";

/// Trace field value identifying a member-removal transition.
pub const TRANSITION_REMOVAL: &str = "removal";

/// Trace field value identifying a shutdown-preparing transition.
pub const TRANSITION_SHUTDOWN_PREPARING: &str = "shutdown_preparing";

/// Trace field value identifying a shutdown-ready transition.
pub const TRANSITION_SHUTDOWN_READY: &str = "shutdown_ready";

/// Trace field value identifying a data-center-unreachable transition.
pub const TRANSITION_DC_UNREACHABLE: &str = "dc_unreachable";

/// Trace field value identifying a data-center-reachable transition.
pub const TRANSITION_DC_REACHABLE: &str = "dc_reachable";

// --- メンバー識別・data center フィールド名定数（要件 4.2） ---

/// Structured trace field name for the node identifier of the affected member.
pub const FIELD_NODE_ID: &str = "node_id";

/// Structured trace field name for the authority address of the affected member.
pub const FIELD_AUTHORITY: &str = "authority";

/// Structured trace field name for the data center identifier.
pub const FIELD_DATA_CENTER: &str = "data_center";

/// Structured trace field name for the lifecycle transition kind.
///
/// Use this key together with one of the `TRANSITION_*` constants as the value.
pub const FIELD_TRANSITION: &str = "cluster.lifecycle.transition";
