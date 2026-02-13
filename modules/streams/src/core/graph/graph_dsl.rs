use super::{Flow, Sink};

#[cfg(test)]
mod tests;

/// Partial graph builder equivalent for composing reusable flow fragments.
pub struct GraphDsl<In, Out, Mat> {
  flow: Flow<In, Out, Mat>,
}

impl<In, Out, Mat> GraphDsl<In, Out, Mat> {
  /// Creates a DSL builder from an existing flow fragment.
  #[must_use]
  pub const fn from_flow(flow: Flow<In, Out, Mat>) -> Self {
    Self { flow }
  }

  /// Appends a flow fragment to this partial graph.
  #[must_use]
  pub fn via<T, Mat2>(self, flow: Flow<Out, T, Mat2>) -> GraphDsl<In, T, Mat>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static,
    T: Send + Sync + 'static, {
    GraphDsl { flow: self.flow.via(flow) }
  }

  /// Connects the partial graph to a sink.
  #[must_use]
  pub fn to<Mat2>(self, sink: Sink<Out, Mat2>) -> Sink<In, Mat>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static, {
    self.flow.to(sink)
  }

  /// Finalizes and returns a reusable flow fragment.
  #[must_use]
  pub fn build(self) -> Flow<In, Out, Mat> {
    self.flow
  }
}
