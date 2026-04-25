use alloc::vec::Vec;

use super::{buffered_edge::BufferedEdge, outlet_dispatch_state::OutletDispatchState};
use crate::core::{DynValue, StreamError, shape::PortId};

/// Runtime connection table for graph interpreter edges.
pub(in crate::core) struct GraphConnections {
  edges:    Vec<BufferedEdge>,
  dispatch: Vec<OutletDispatchState>,
}

impl GraphConnections {
  /// Creates a connection table from compiled edges and dispatch cursors.
  #[must_use]
  pub(in crate::core) const fn new(edges: Vec<BufferedEdge>, dispatch: Vec<OutletDispatchState>) -> Self {
    Self { edges, dispatch }
  }

  /// Returns all buffered edges.
  #[must_use]
  pub(in crate::core) fn edges(&self) -> &[BufferedEdge] {
    &self.edges
  }

  /// Returns whether an outlet has buffered outgoing data.
  #[must_use]
  pub(in crate::core) fn has_buffered_outgoing(&self, from: PortId) -> bool {
    self.edges.iter().any(|edge| edge.from() == from && !edge.is_empty())
  }

  /// Polls one buffered element from incoming edges with an optional slot preference.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the underlying edge buffer rejects a poll.
  pub(in crate::core) fn poll_incoming_with_preferred(
    &mut self,
    to: PortId,
    preferred_slot: Option<usize>,
  ) -> Result<Option<(usize, DynValue)>, StreamError> {
    let incoming_count = self.edges.iter().filter(|edge| edge.to() == to).count();
    if incoming_count == 0 {
      return Ok(None);
    }

    let mut preferred_checked = false;
    if let Some(slot) = preferred_slot
      && let Some(edge_index) = self.nth_incoming_index(to, slot)
    {
      preferred_checked = true;
      if !self.edges[edge_index].is_empty() {
        let Some(value) = self.edges[edge_index].poll()? else {
          return Err(StreamError::InvalidConnection);
        };
        return Ok(Some((slot, value)));
      }
    }

    let skipped_slots = if preferred_checked { 1 } else { 0 };
    let start_slot = preferred_slot.map(|slot| slot + 1).unwrap_or(0);
    for offset in 0..incoming_count.saturating_sub(skipped_slots) {
      let slot = (start_slot + offset) % incoming_count;
      if let Some(edge_index) = self.nth_incoming_index(to, slot)
        && !self.edges[edge_index].is_empty()
      {
        let Some(value) = self.edges[edge_index].poll()? else {
          return Err(StreamError::InvalidConnection);
        };
        return Ok(Some((slot, value)));
      }
    }

    Ok(None)
  }

  /// Offers an element to the next outgoing edge for the outlet.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when there is no outgoing edge or the selected
  /// edge buffer cannot accept the element.
  pub(in crate::core) fn offer_next(&mut self, from: PortId, value: DynValue) -> Result<(), StreamError> {
    let target = self.next_outgoing_edge_index(from)?;
    self.offer_at(target, value)
  }

  /// Offers an element to a specific edge.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the edge buffer cannot accept the element.
  pub(in crate::core) fn offer_at(&mut self, edge_index: usize, value: DynValue) -> Result<(), StreamError> {
    self.edges[edge_index].offer(value)
  }

  /// Returns outgoing edge indices for the outlet.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::InvalidConnection`] when no outgoing edge exists.
  pub(in crate::core) fn outgoing_edge_indices(&self, from: PortId) -> Result<Vec<usize>, StreamError> {
    let mut outgoing_edges = Vec::new();
    for (index, edge) in self.edges.iter().enumerate() {
      if edge.from() == from {
        outgoing_edges.push(index);
      }
    }
    if outgoing_edges.is_empty() {
      return Err(StreamError::InvalidConnection);
    }
    Ok(outgoing_edges)
  }

  /// Returns incoming edge indices for the inlet.
  #[must_use]
  pub(in crate::core) fn incoming_edge_indices(&self, to: PortId) -> Vec<usize> {
    self.edges.iter().enumerate().filter_map(|(index, edge)| (edge.to() == to).then_some(index)).collect()
  }

  /// Returns whether all edge buffers are empty.
  #[must_use]
  pub(in crate::core) fn all_buffers_empty(&self) -> bool {
    self.edges.iter().all(BufferedEdge::is_empty)
  }

  /// Returns the upstream outlet for an edge index.
  #[must_use]
  pub(in crate::core) fn edge_from(&self, edge_index: usize) -> PortId {
    self.edges[edge_index].from()
  }

  /// Returns the downstream inlet for an edge index.
  #[must_use]
  pub(in crate::core) fn edge_to(&self, edge_index: usize) -> PortId {
    self.edges[edge_index].to()
  }

  /// Returns whether an edge is closed.
  #[must_use]
  pub(in crate::core) fn edge_closed(&self, edge_index: usize) -> bool {
    self.edges[edge_index].is_closed()
  }

  /// Returns whether an edge is closed and empty.
  #[must_use]
  pub(in crate::core) fn edge_closed_and_empty(&self, edge_index: usize) -> bool {
    self.edges[edge_index].is_closed() && self.edges[edge_index].is_empty()
  }

  /// Returns whether all outgoing edges for an outlet are closed.
  #[must_use]
  pub(in crate::core) fn all_outgoing_closed(&self, outlet: PortId) -> bool {
    self.edges.iter().filter(|edge| edge.from() == outlet).all(BufferedEdge::is_closed)
  }

  /// Closes all outgoing edges for an outlet.
  pub(in crate::core) fn close_outgoing(&mut self, outlet: PortId) {
    for edge in &mut self.edges {
      if edge.from() == outlet {
        edge.close();
      }
    }
  }

  /// Closes and clears all incoming edges for an inlet.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when an edge buffer rejects a poll during drain.
  pub(in crate::core) fn close_and_clear_incoming(&mut self, inlet: PortId) -> Result<(), StreamError> {
    let incoming_edges = self.incoming_edge_indices(inlet);
    for edge_index in incoming_edges {
      self.edges[edge_index].close_and_clear()?;
    }
    Ok(())
  }

  fn next_outgoing_edge_index(&mut self, from: PortId) -> Result<usize, StreamError> {
    let outgoing_edges = self.outgoing_edge_indices(from)?;
    let Some(state_index) = self.dispatch.iter().position(|state| state.outlet() == from) else {
      return Err(StreamError::InvalidConnection);
    };
    let selected_slot = self.dispatch[state_index].select_next(outgoing_edges.len())?;
    Ok(outgoing_edges[selected_slot])
  }

  fn nth_incoming_index(&self, to: PortId, slot: usize) -> Option<usize> {
    self.edges.iter().enumerate().filter(|(_, edge)| edge.to() == to).nth(slot).map(|(index, _)| index)
  }
}
