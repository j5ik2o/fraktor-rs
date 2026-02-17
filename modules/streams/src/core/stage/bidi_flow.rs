use super::{Flow, StreamNotUsed};

#[cfg(test)]
mod tests;

/// Minimal bidirectional flow representation.
pub struct BidiFlow<InTop, OutTop, InBottom, OutBottom> {
  top:    Flow<InTop, OutTop, StreamNotUsed>,
  bottom: Flow<InBottom, OutBottom, StreamNotUsed>,
}

impl<T> BidiFlow<T, T, T, T> {
  /// Creates an identity bidirectional flow that passes elements through unchanged.
  #[must_use]
  pub fn identity() -> Self
  where
    T: Send + Sync + 'static, {
    Self { top: Flow::new(), bottom: Flow::new() }
  }
}

impl<InTop, OutTop, InBottom, OutBottom> BidiFlow<InTop, OutTop, InBottom, OutBottom> {
  /// Creates a bidirectional flow from top and bottom flow fragments.
  #[must_use]
  pub const fn from_flows(
    top: Flow<InTop, OutTop, StreamNotUsed>,
    bottom: Flow<InBottom, OutBottom, StreamNotUsed>,
  ) -> Self {
    Self { top, bottom }
  }

  /// Splits the bidirectional flow into top and bottom flow fragments.
  #[must_use]
  pub fn split(self) -> (Flow<InTop, OutTop, StreamNotUsed>, Flow<InBottom, OutBottom, StreamNotUsed>) {
    (self.top, self.bottom)
  }

  /// Reverses this bidirectional flow, swapping the top and bottom fragments.
  #[must_use]
  pub fn reversed(self) -> BidiFlow<InBottom, OutBottom, InTop, OutTop> {
    BidiFlow { top: self.bottom, bottom: self.top }
  }
}
