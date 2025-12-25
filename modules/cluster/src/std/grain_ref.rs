//! Std wrapper for grain references.

use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::core::GrainRefGeneric;

/// Grain reference bound to the standard toolbox.
pub type GrainRef = GrainRefGeneric<StdToolbox>;
