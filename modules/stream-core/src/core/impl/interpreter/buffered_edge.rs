use crate::core::{
  DynValue, StreamError,
  r#impl::fusing::{StreamBuffer, StreamBufferConfig},
  materialization::MatCombine,
  shape::PortId,
  snapshot::ConnectionState,
};

/// Buffered runtime edge between two stream ports.
pub(in crate::core) struct BufferedEdge {
  from:   PortId,
  to:     PortId,
  _mat:   MatCombine,
  closed: bool,
  buffer: StreamBuffer<DynValue>,
}

impl BufferedEdge {
  /// Creates a buffered edge for a compiled stream graph.
  #[must_use]
  pub(in crate::core) fn new(from: PortId, to: PortId, mat: MatCombine, buffer_config: StreamBufferConfig) -> Self {
    Self { from, to, _mat: mat, closed: false, buffer: StreamBuffer::new(buffer_config) }
  }

  /// Returns the upstream outlet port.
  #[must_use]
  pub(in crate::core) const fn from(&self) -> PortId {
    self.from
  }

  /// Returns the downstream inlet port.
  #[must_use]
  pub(in crate::core) const fn to(&self) -> PortId {
    self.to
  }

  /// Returns whether this edge is closed.
  #[must_use]
  pub(in crate::core) const fn is_closed(&self) -> bool {
    self.closed
  }

  /// Returns whether this edge has no buffered element.
  #[must_use]
  pub(in crate::core) fn is_empty(&self) -> bool {
    self.buffer.is_empty()
  }

  /// Returns the diagnostic connection state for this edge.
  #[must_use]
  pub(in crate::core) fn connection_state(&self) -> ConnectionState {
    if self.closed {
      ConnectionState::Closed
    } else if self.buffer.is_empty() {
      ConnectionState::ShouldPull
    } else {
      ConnectionState::ShouldPush
    }
  }

  /// Offers an element to the edge buffer.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::BufferOverflow`] when the edge buffer cannot
  /// accept the element.
  pub(in crate::core) fn offer(&mut self, value: DynValue) -> Result<(), StreamError> {
    if self.closed {
      return Ok(());
    }
    match self.buffer.offer(value) {
      | Ok(_) => Ok(()),
      | Err(error) => Err(error),
    }
  }

  /// Polls one buffered element from this edge.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the underlying buffer rejects the poll.
  pub(in crate::core) fn poll(&mut self) -> Result<Option<DynValue>, StreamError> {
    if self.buffer.is_empty() {
      return Ok(None);
    }
    Ok(Some(self.buffer.poll()?))
  }

  /// Closes this edge.
  pub(in crate::core) const fn close(&mut self) {
    self.closed = true;
  }

  /// Closes this edge and drains buffered elements.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the underlying buffer rejects a poll.
  pub(in crate::core) fn close_and_clear(&mut self) -> Result<(), StreamError> {
    self.close();
    while self.poll()?.is_some() {}
    Ok(())
  }
}
