use super::Attribute;

/// Marker trait for attributes that are always present on a materialized stage.
///
/// Mirrors Pekko's `sealed trait MandatoryAttribute extends Attribute`.
/// Whereas Pekko enforces membership at runtime via the sealed hierarchy,
/// the Rust translation relies on the trait bound: only types that
/// explicitly implement `MandatoryAttribute` can be requested via
/// `Attributes::mandatory_attribute::<T>()`, yielding a compile-time
/// equivalent of the Scala-side guarantee.
pub trait MandatoryAttribute: Attribute {}
