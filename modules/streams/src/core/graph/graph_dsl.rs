use super::{Flow, Sink, StreamDslError};

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

  /// Adds a broadcast fan-out stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_out` is zero.
  pub fn broadcast(self, fan_out: usize) -> Result<GraphDsl<In, Out, Mat>, StreamDslError>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + Clone + 'static, {
    Ok(GraphDsl { flow: self.flow.broadcast(fan_out)? })
  }

  /// Adds a balance fan-out stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_out` is zero.
  pub fn balance(self, fan_out: usize) -> Result<GraphDsl<In, Out, Mat>, StreamDslError>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static, {
    Ok(GraphDsl { flow: self.flow.balance(fan_out)? })
  }

  /// Adds a merge fan-in stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn merge(self, fan_in: usize) -> Result<GraphDsl<In, Out, Mat>, StreamDslError>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static, {
    Ok(GraphDsl { flow: self.flow.merge(fan_in)? })
  }

  /// Adds a concat fan-in stage.
  ///
  /// # Errors
  ///
  /// Returns [`StreamDslError`] when `fan_in` is zero.
  pub fn concat(self, fan_in: usize) -> Result<GraphDsl<In, Out, Mat>, StreamDslError>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static, {
    Ok(GraphDsl { flow: self.flow.concat(fan_in)? })
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
