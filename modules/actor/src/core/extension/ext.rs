//! Actor system extension trait.

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

/// Marker trait implemented by every actor-system extension.
pub trait Extension<TB: RuntimeToolbox>: Send + Sync + 'static {}
