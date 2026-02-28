//! Strategy applied when stashing fails while fencing commands.

#[cfg(test)]
mod tests;

/// Strategy applied when command stashing cannot proceed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StashOverflowStrategy {
  /// Drop the incoming command and continue.
  Drop,
  /// Fail message handling with an error.
  Fail,
}
