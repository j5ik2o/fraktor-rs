//! Typed distributed data key for version vector values.

use super::{Key, VersionVector};

/// Key for [`VersionVector`] values.
pub type VersionVectorKey = Key<VersionVector>;
