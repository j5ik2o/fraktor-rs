//! Stream attributes used to annotate stages and graphs.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::String, vec::Vec};

use super::{AsyncBoundaryAttr, Attribute, DispatcherAttribute, InputBuffer, LogLevel, LogLevels};

/// Immutable collection of stream attributes.
///
/// Supports both named string attributes (legacy) and typed
/// [`Attribute`] trait objects with downcast-based retrieval.
#[derive(Debug)]
pub struct Attributes {
  names: Vec<String>,
  attrs: Vec<Box<dyn Attribute>>,
}

impl Attributes {
  /// Creates an empty attributes collection.
  #[must_use]
  pub const fn new() -> Self {
    Self { names: Vec::new(), attrs: Vec::new() }
  }

  /// Creates attributes containing a single stage name.
  #[must_use]
  pub fn named(name: impl Into<String>) -> Self {
    Self { names: alloc::vec![name.into()], attrs: Vec::new() }
  }

  /// Creates attributes with an [`InputBuffer`] configuration.
  #[must_use]
  pub fn input_buffer(initial: usize, max: usize) -> Self {
    Self {
      names: alloc::vec![String::from("input-buffer")],
      attrs: alloc::vec![Box::new(InputBuffer::new(initial, max))],
    }
  }

  /// Creates attributes with a [`LogLevels`] configuration.
  #[must_use]
  pub fn log_levels(on_element: LogLevel, on_finish: LogLevel, on_failure: LogLevel) -> Self {
    Self {
      names: alloc::vec![String::from("log-levels")],
      attrs: alloc::vec![Box::new(LogLevels::new(on_element, on_finish, on_failure))],
    }
  }

  /// Creates attributes containing an [`AsyncBoundaryAttr`] marker.
  ///
  /// Mirrors Pekko's `Attributes.asyncBoundary`.
  #[must_use]
  pub fn async_boundary() -> Self {
    Self { names: alloc::vec![String::from("async-boundary")], attrs: alloc::vec![Box::new(AsyncBoundaryAttr)] }
  }

  /// Creates attributes containing a [`DispatcherAttribute`].
  ///
  /// A dispatcher attribute implies an async boundary; the materializer
  /// uses the named dispatcher for the resulting island.
  #[must_use]
  pub fn dispatcher(name: impl Into<String>) -> Self {
    Self {
      names: alloc::vec![String::from("dispatcher")],
      attrs: alloc::vec![Box::new(DispatcherAttribute::new(name))],
    }
  }

  /// Appends names and typed attributes from another collection.
  #[must_use]
  pub fn and(mut self, other: Self) -> Self {
    self.names.extend(other.names);
    self.attrs.extend(other.attrs);
    self
  }

  /// Retrieves a typed attribute by its concrete type.
  ///
  /// Returns `None` if no attribute of type `T` is stored.
  #[must_use]
  pub fn get<T: Attribute + 'static>(&self) -> Option<&T> {
    self.attrs.iter().find_map(|attr| attr.as_any().downcast_ref::<T>())
  }

  /// Returns all configured stage names.
  #[must_use]
  pub fn names(&self) -> &[String] {
    &self.names
  }

  /// Returns `true` when no attributes have been configured.
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.names.is_empty() && self.attrs.is_empty()
  }

  /// Returns `true` when these attributes indicate an async boundary.
  ///
  /// An async boundary is indicated by either an [`AsyncBoundaryAttr`]
  /// or a [`DispatcherAttribute`] (which implies an async boundary).
  /// This mirrors Pekko's `Attributes.isAsync` logic.
  #[must_use]
  pub fn is_async(&self) -> bool {
    self.get::<AsyncBoundaryAttr>().is_some() || self.get::<DispatcherAttribute>().is_some()
  }
}

impl Default for Attributes {
  fn default() -> Self {
    Self::new()
  }
}

impl Clone for Attributes {
  fn clone(&self) -> Self {
    Self { names: self.names.clone(), attrs: self.attrs.iter().map(|attr| attr.clone_box()).collect() }
  }
}

impl PartialEq for Attributes {
  fn eq(&self, other: &Self) -> bool {
    self.names == other.names
      && self.attrs.len() == other.attrs.len()
      && self.attrs.iter().zip(other.attrs.iter()).all(|(a, b)| a.eq_attr(b.as_any()))
  }
}

impl Eq for Attributes {}
