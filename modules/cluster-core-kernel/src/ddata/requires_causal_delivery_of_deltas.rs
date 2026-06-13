//! Marker for deltas that require causal delivery.

use super::ReplicatedDelta;

/// Marker trait for deltas whose delivery must preserve causal order.
pub trait RequiresCausalDeliveryOfDeltas: ReplicatedDelta {}
