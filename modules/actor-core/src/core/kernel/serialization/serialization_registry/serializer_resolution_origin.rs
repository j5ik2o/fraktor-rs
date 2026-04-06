//! Serializer resolution origin indicator.

/// Indicates how a serializer was resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerializerResolutionOrigin {
  /// Resolution returned a cached serializer.
  Cache,
  /// Resolution came from an explicit binding.
  Binding,
  /// Resolution fell back to the default serializer.
  Fallback,
}
