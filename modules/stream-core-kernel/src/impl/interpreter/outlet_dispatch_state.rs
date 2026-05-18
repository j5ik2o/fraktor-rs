use crate::{StreamError, shape::PortId};

/// Round-robin cursor for one outlet.
pub(crate) struct OutletDispatchState {
  outlet:    PortId,
  next_edge: usize,
}

impl OutletDispatchState {
  /// Creates a dispatch cursor for the given outlet.
  #[must_use]
  pub(crate) const fn new(outlet: PortId) -> Self {
    Self { outlet, next_edge: 0 }
  }

  /// Returns the outlet associated with this cursor.
  #[must_use]
  pub(crate) const fn outlet(&self) -> PortId {
    self.outlet
  }

  /// Advances the cursor and returns the selected edge slot.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] when there are no outgoing
  /// edges for the outlet.
  pub(crate) const fn select_next(&mut self, edge_count: usize) -> Result<usize, StreamError> {
    if edge_count == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let selected = self.next_edge % edge_count;
    self.next_edge = (selected + 1) % edge_count;
    Ok(selected)
  }
}
