use alloc::vec::Vec;

#[cfg(test)]
#[path = "stateful_map_concat_accumulator_test.rs"]
mod tests;

/// Accumulator for `stateful_map_concat` that supports emitting
/// additional elements when the upstream completes.
///
/// Implement [`apply`](Self::apply) to transform each input element into
/// zero or more output elements, and optionally override
/// [`on_complete`](Self::on_complete) to emit trailing elements after the
/// upstream finishes.
pub trait StatefulMapConcatAccumulator<In, Out>: Send + Sync {
  /// Transforms an input element into zero or more output elements.
  fn apply(&mut self, input: In) -> Vec<Out>;

  /// Called once when the upstream completes.
  ///
  /// Returns additional elements to emit before the stream finishes.
  /// The default implementation emits nothing.
  fn on_complete(&mut self) -> Vec<Out> {
    Vec::new()
  }
}
