#[cfg(test)]
mod tests;

use alloc::{boxed::Box, format, string::String};
use core::marker::PhantomData;

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::SubSourceOutletHandler;
use crate::core::{
  DynValue, SourceLogic, StreamError,
  dsl::Source,
  materialization::StreamNotUsed,
  stage::{CancellationCause, StageKind},
};

struct SubSourceOutletState<T> {
  name:      String,
  pending:   Option<T>,
  available: bool,
  closed:    bool,
  failure:   Option<StreamError>,
  handler:   Option<Box<dyn SubSourceOutletHandler<T>>>,
}

impl<T> SubSourceOutletState<T> {
  fn new(name: &str) -> Self {
    Self {
      name:      name.into(),
      pending:   None,
      available: false,
      closed:    false,
      failure:   None,
      handler:   None,
    }
  }

  fn port_error(&self, action: &'static str) -> StreamError {
    StreamError::failed_with_context(format!("SubSourceOutlet({}) cannot {action}", self.name))
  }
}

/// Dynamic output port connected to a materializable substream source.
pub struct SubSourceOutlet<T> {
  state: ArcShared<SpinSyncMutex<SubSourceOutletState<T>>>,
}

impl<T> SubSourceOutlet<T> {
  /// Creates a new sub-source outlet.
  #[must_use]
  pub fn new(name: &str) -> Self {
    Self { state: ArcShared::new(SpinSyncMutex::new(SubSourceOutletState::new(name))) }
  }

  /// Sets the event handler for this dynamic output port.
  pub fn set_handler<H>(&mut self, handler: H)
  where
    H: SubSourceOutletHandler<T> + 'static, {
    let mut guard = self.state.lock();
    guard.handler = Some(Box::new(handler));
  }

  /// Returns `true` when downstream demand allows a push.
  #[must_use]
  pub fn is_available(&self) -> bool {
    let guard = self.state.lock();
    guard.available
  }

  /// Returns `true` when the outlet has observed terminal state.
  #[must_use]
  pub fn is_closed(&self) -> bool {
    let guard = self.state.lock();
    guard.closed
  }

  /// Pushes one element to the attached substream source.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when downstream has not pulled, the port is
  /// already closed, or a previous element has not been consumed.
  pub fn push(&mut self, value: T) -> Result<(), StreamError> {
    let mut guard = self.state.lock();
    if guard.closed {
      return Err(guard.port_error("push because it is closed"));
    }
    if !guard.available || guard.pending.is_some() {
      return Err(guard.port_error("push before pull"));
    }
    guard.pending = Some(value);
    guard.available = false;
    Ok(())
  }

  /// Completes the dynamic output port.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the port has already terminated.
  pub fn complete(&mut self) -> Result<(), StreamError> {
    let mut guard = self.state.lock();
    if guard.closed {
      return Err(guard.port_error("complete because it is already closed"));
    }
    guard.available = false;
    guard.closed = true;
    Ok(())
  }

  /// Fails the dynamic output port.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the port has already terminated.
  pub fn fail(&mut self, error: StreamError) -> Result<(), StreamError> {
    let mut guard = self.state.lock();
    if guard.closed {
      return Err(guard.port_error("fail because it is already closed"));
    }
    guard.pending = None;
    guard.available = false;
    guard.closed = true;
    guard.failure = Some(error);
    Ok(())
  }
}

impl<T> SubSourceOutlet<T>
where
  T: Send + 'static,
{
  /// Returns the source endpoint for this dynamic output port.
  #[must_use]
  pub fn source(&self) -> Source<T, StreamNotUsed> {
    let logic = SubSourceOutletLogic::<T> { state: self.state.clone(), _pd: PhantomData };
    Source::from_logic(StageKind::Custom, logic)
  }
}

struct SubSourceOutletLogic<T> {
  state: ArcShared<SpinSyncMutex<SubSourceOutletState<T>>>,
  _pd:   PhantomData<fn() -> T>,
}

impl<T> SubSourceOutletLogic<T> {
  fn take_handler(&self) -> Option<Box<dyn SubSourceOutletHandler<T>>> {
    let mut guard = self.state.lock();
    guard.handler.take()
  }

  fn restore_handler(&self, handler: Box<dyn SubSourceOutletHandler<T>>) {
    let mut guard = self.state.lock();
    if guard.handler.is_none() {
      guard.handler = Some(handler);
    }
  }
}

impl<T> SourceLogic for SubSourceOutletLogic<T>
where
  T: Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    {
      let mut guard = self.state.lock();
      if let Some(error) = &guard.failure {
        return Err(error.clone());
      }
      if let Some(value) = guard.pending.take() {
        return Ok(Some(Box::new(value) as DynValue));
      }
      if guard.closed {
        return Ok(None);
      }
      if guard.available {
        return Err(StreamError::WouldBlock);
      }
      guard.available = true;
    }

    let Some(mut handler) = self.take_handler() else {
      return Err(StreamError::WouldBlock);
    };
    let result = handler.on_pull();
    self.restore_handler(handler);
    match result {
      | Ok(()) => Err(StreamError::WouldBlock),
      | Err(error) => {
        let mut guard = self.state.lock();
        guard.available = false;
        guard.closed = true;
        guard.failure = Some(error.clone());
        Err(error)
      },
    }
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    {
      let mut guard = self.state.lock();
      if guard.closed {
        return Ok(());
      }
      guard.available = false;
      guard.closed = true;
      guard.pending = None;
    }
    let Some(mut handler) = self.take_handler() else {
      return Ok(());
    };
    let result = handler.on_downstream_finish(CancellationCause::no_more_elements_needed());
    self.restore_handler(handler);
    result
  }
}
