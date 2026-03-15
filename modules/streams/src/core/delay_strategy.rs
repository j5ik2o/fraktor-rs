#[cfg(test)]
mod tests;

/// Strategy for computing per-element delay in tick units.
///
/// Instances are not shared among running streams; all elements pass
/// through [`next_delay`](Self::next_delay) sequentially, so the
/// implementation may be stateful.
pub trait DelayStrategy<T>: Send + Sync {
  /// Returns the delay in ticks for the given element.
  ///
  /// A return value of `0` means the element passes without delay.
  fn next_delay(&mut self, elem: &T) -> u64;
}
