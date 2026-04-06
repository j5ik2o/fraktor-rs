//! Pekko-compatible alias for [`ExtensionSetup`].
//!
//! In Pekko, `AbstractExtensionSetup[T]` is a thin base class that bridges
//! Scala function types and Java `Function` types for extension factories.
//! Rust closures eliminate that language-gap concern, so this module simply
//! re-exports [`ExtensionSetup`] under the Pekko name for discoverability.

use super::extension_setup::ExtensionSetup;

/// Pekko-compatible alias for [`ExtensionSetup`].
///
/// Corresponds to `org.apache.pekko.actor.typed.Extensions.AbstractExtensionSetup`.
pub type AbstractExtensionSetup<I> = ExtensionSetup<I>;
