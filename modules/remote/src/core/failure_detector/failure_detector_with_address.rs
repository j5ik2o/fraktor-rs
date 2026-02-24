/// Extension trait for failure detectors that can be bound to an address after creation.
///
/// Mirrors Pekko's `FailureDetectorWithAddress` — the address of the observed host
/// is set after detector construction so that a single factory can produce detectors
/// before the remote address is known.
pub trait FailureDetectorWithAddress {
  /// Binds the detector to the given address string.
  fn set_address(&mut self, addr: &str);
}
