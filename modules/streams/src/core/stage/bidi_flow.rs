use super::{Flow, StreamNotUsed};

#[cfg(test)]
mod tests;

/// Minimal bidirectional flow representation.
pub struct BidiFlow<InTop, OutTop, InBottom, OutBottom, Mat> {
  top:    Flow<InTop, OutTop, StreamNotUsed>,
  bottom: Flow<InBottom, OutBottom, StreamNotUsed>,
  mat:    Mat,
}

impl<T> BidiFlow<T, T, T, T, StreamNotUsed> {
  /// Creates an identity bidirectional flow that passes elements through unchanged.
  #[must_use]
  pub fn identity() -> Self
  where
    T: Send + Sync + 'static, {
    Self::from_flows(Flow::new(), Flow::new())
  }
}

impl<InTop, OutTop, InBottom, OutBottom> BidiFlow<InTop, OutTop, InBottom, OutBottom, StreamNotUsed>
where
  InTop: Send + Sync + 'static,
  OutTop: Send + Sync + 'static,
  InBottom: Send + Sync + 'static,
  OutBottom: Send + Sync + 'static,
{
  /// Creates a bidirectional flow from top and bottom flow fragments.
  #[must_use]
  pub const fn from_flows(
    top: Flow<InTop, OutTop, StreamNotUsed>,
    bottom: Flow<InBottom, OutBottom, StreamNotUsed>,
  ) -> Self {
    Self { top, bottom, mat: StreamNotUsed::new() }
  }

  /// Creates a bidirectional flow from mapping functions.
  #[must_use]
  pub fn from_function<FOut, FIn>(outbound: FOut, inbound: FIn) -> Self
  where
    FOut: Fn(InTop) -> OutTop + Send + Sync + 'static,
    FIn: Fn(InBottom) -> OutBottom + Send + Sync + 'static, {
    Self::from_functions(outbound, inbound)
  }

  /// Creates a bidirectional flow from mapping functions.
  #[must_use]
  pub fn from_functions<FOut, FIn>(outbound: FOut, inbound: FIn) -> Self
  where
    FOut: Fn(InTop) -> OutTop + Send + Sync + 'static,
    FIn: Fn(InBottom) -> OutBottom + Send + Sync + 'static, {
    Self::from_flows(Flow::from_function(outbound), Flow::from_function(inbound))
  }
}

impl<InTop, OutTop, InBottom, OutBottom, Mat> BidiFlow<InTop, OutTop, InBottom, OutBottom, Mat>
where
  InTop: Send + Sync + 'static,
  OutTop: Send + Sync + 'static,
  InBottom: Send + Sync + 'static,
  OutBottom: Send + Sync + 'static,
{
  /// Creates a bidirectional flow from top/bottom fragments and materialized value.
  #[must_use]
  pub const fn from_flows_mat(
    top: Flow<InTop, OutTop, StreamNotUsed>,
    bottom: Flow<InBottom, OutBottom, StreamNotUsed>,
    mat: Mat,
  ) -> Self {
    Self { top, bottom, mat }
  }

  /// Splits the bidirectional flow into top and bottom flow fragments.
  #[must_use]
  pub fn split(self) -> (Flow<InTop, OutTop, StreamNotUsed>, Flow<InBottom, OutBottom, StreamNotUsed>) {
    (self.top, self.bottom)
  }

  /// Reverses this bidirectional flow, swapping the top and bottom fragments.
  #[must_use]
  pub fn reversed(self) -> BidiFlow<InBottom, OutBottom, InTop, OutTop, Mat> {
    BidiFlow { top: self.bottom, bottom: self.top, mat: self.mat }
  }

  /// Stacks another bidirectional flow on top of this one.
  #[must_use]
  pub fn atop<OutTop2, InBottom2, Mat2>(
    self,
    bidi: BidiFlow<OutTop, OutTop2, InBottom2, InBottom, Mat2>,
  ) -> BidiFlow<InTop, OutTop2, InBottom2, OutBottom, Mat>
  where
    OutTop2: Send + Sync + 'static,
    InBottom2: Send + Sync + 'static, {
    BidiFlow { top: self.top.via(bidi.top), bottom: bidi.bottom.via(self.bottom), mat: self.mat }
  }

  /// Joins this bidirectional flow with the provided flow.
  #[must_use]
  pub fn join<Mat2>(self, flow: Flow<OutTop, InBottom, Mat2>) -> Flow<InTop, OutBottom, Mat> {
    let (graph, _ignored) = self.top.via(flow).via(self.bottom).into_parts();
    Flow::from_graph(graph, self.mat)
  }
}
