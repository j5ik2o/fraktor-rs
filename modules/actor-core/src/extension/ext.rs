//! Actor system extension trait.

use crate::RuntimeToolbox;

/// Marker trait implemented by every actor-system extension.
pub trait Extension<TB: RuntimeToolbox>: Send + Sync + 'static {}
