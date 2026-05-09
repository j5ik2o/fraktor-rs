#[cfg(test)]
mod tests;

use alloc::{boxed::Box, format, string::String};
use core::marker::PhantomData;

use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

use super::SubSinkInletHandler;
use crate::{
  DemandTracker, DynValue, SinkDecision, SinkLogic, StreamError, downcast_value, dsl::Sink,
  materialization::StreamNotUsed, stage::StageKind,
};

struct SubSinkInletState<T> {
  name:             String,
  element:          Option<T>,
  closed:           bool,
  pulled:           bool,
  demand_requested: bool,
  failure:          Option<StreamError>,
  handler:          Option<Box<dyn SubSinkInletHandler<T>>>,
}

impl<T> SubSinkInletState<T> {
  fn new(name: &str) -> Self {
    Self {
      name:             name.into(),
      element:          None,
      closed:           false,
      pulled:           false,
      demand_requested: false,
      failure:          None,
      handler:          None,
    }
  }

  fn port_error(&self, action: &'static str) -> StreamError {
    StreamError::failed_with_context(format!("SubSinkInlet({}) cannot {action}", self.name))
  }
}

/// Dynamic input port connected to a materializable substream sink.
pub struct SubSinkInlet<T> {
  state: ArcShared<SpinSyncMutex<SubSinkInletState<T>>>,
}

impl<T> SubSinkInlet<T> {
  /// Creates a new sub-sink inlet.
  #[must_use]
  pub fn new(name: &str) -> Self {
    Self { state: ArcShared::new(SpinSyncMutex::new(SubSinkInletState::new(name))) }
  }

  /// Sets the event handler for this dynamic input port.
  pub fn set_handler<H>(&mut self, handler: H)
  where
    H: SubSinkInletHandler<T> + 'static, {
    let mut guard = self.state.lock();
    guard.handler = Some(Box::new(handler));
  }

  /// Returns `true` when an element is ready to be grabbed.
  #[must_use]
  pub fn is_available(&self) -> bool {
    let guard = self.state.lock();
    guard.element.is_some()
  }

  /// Returns `true` when the inlet has observed terminal state.
  #[must_use]
  pub fn is_closed(&self) -> bool {
    let guard = self.state.lock();
    guard.closed
  }

  /// Returns `true` when a pull is pending on the inlet.
  #[must_use]
  pub fn has_been_pulled(&self) -> bool {
    let guard = self.state.lock();
    guard.pulled && !guard.closed
  }

  /// Grabs the currently available element.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError::WouldBlock`] when no element has arrived.
  pub fn grab(&mut self) -> Result<T, StreamError> {
    let mut guard = self.state.lock();
    match guard.element.take() {
      | Some(value) => Ok(value),
      | None => Err(StreamError::WouldBlock),
    }
  }

  /// Pulls one element from the attached substream sink.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the port is already pulled, closed, or still
  /// has an unconsumed element.
  pub fn pull(&mut self) -> Result<(), StreamError> {
    let mut guard = self.state.lock();
    if guard.closed {
      return Err(guard.port_error("pull because it is closed"));
    }
    if guard.pulled {
      return Err(guard.port_error("pull twice"));
    }
    if guard.element.is_some() {
      return Err(guard.port_error("pull before grabbing the current element"));
    }
    guard.pulled = true;
    guard.demand_requested = false;
    Ok(())
  }

  /// Cancels the dynamic input port.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the port is already closed.
  pub fn cancel(&mut self) -> Result<(), StreamError> {
    let mut guard = self.state.lock();
    if guard.closed {
      return Err(guard.port_error("cancel because it is already closed"));
    }
    guard.closed = true;
    guard.pulled = false;
    guard.demand_requested = false;
    guard.element = None;
    Ok(())
  }
}

impl<T> SubSinkInlet<T>
where
  T: Send + Sync + 'static,
{
  /// Returns the sink endpoint for this dynamic input port.
  #[must_use]
  pub fn sink(&self) -> Sink<T, StreamNotUsed> {
    let logic = SubSinkInletLogic::<T> { state: self.state.clone(), _pd: PhantomData };
    Sink::from_logic(StageKind::Custom, logic)
  }
}

struct SubSinkInletLogic<T> {
  state: ArcShared<SpinSyncMutex<SubSinkInletState<T>>>,
  _pd:   PhantomData<fn(T)>,
}

impl<T> SubSinkInletLogic<T> {
  fn request_pending_pull(&self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    let mut guard = self.state.lock();
    if let Some(error) = &guard.failure {
      return Err(error.clone());
    }
    if guard.closed || !guard.pulled || guard.element.is_some() || guard.demand_requested {
      return Ok(false);
    }
    demand.request(1)?;
    guard.demand_requested = true;
    Ok(true)
  }

  fn take_handler(&self) -> Result<Box<dyn SubSinkInletHandler<T>>, StreamError> {
    let mut guard = self.state.lock();
    match guard.handler.take() {
      | Some(handler) => Ok(handler),
      | None => Err(guard.port_error("dispatch without a handler")),
    }
  }

  fn restore_handler(&self, handler: Box<dyn SubSinkInletHandler<T>>) {
    let mut guard = self.state.lock();
    if guard.handler.is_none() {
      guard.handler = Some(handler);
    }
  }
}

impl<T> SinkLogic for SubSinkInletLogic<T>
where
  T: Send + 'static,
{
  fn can_accept_input(&self) -> bool {
    let guard = self.state.lock();
    guard.pulled && guard.demand_requested && guard.element.is_none() && !guard.closed
  }

  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    self.request_pending_pull(demand).map(|_| ())
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    let value = downcast_value::<T>(input)?;
    {
      let mut guard = self.state.lock();
      if guard.closed {
        return Err(guard.port_error("accept input because it is closed"));
      }
      if !guard.pulled || guard.element.is_some() {
        return Err(guard.port_error("accept input before pull"));
      }
      guard.element = Some(value);
      guard.pulled = false;
      guard.demand_requested = false;
    }

    let mut handler = self.take_handler()?;
    let result = handler.on_push();
    self.restore_handler(handler);
    result?;
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    let handler = {
      let mut guard = self.state.lock();
      guard.closed = true;
      guard.pulled = false;
      guard.demand_requested = false;
      guard.handler.take()
    };
    if let Some(mut handler) = handler {
      let result = handler.on_upstream_failure(error);
      self.restore_handler(handler);
      if let Err(handler_error) = result {
        let mut guard = self.state.lock();
        guard.failure = Some(handler_error);
      }
    }
  }

  fn on_tick(&mut self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    self.request_pending_pull(demand)
  }

  fn on_upstream_finish(&mut self) -> Result<bool, StreamError> {
    let mut handler = {
      let mut guard = self.state.lock();
      if guard.closed {
        return Ok(false);
      }
      guard.closed = true;
      guard.pulled = false;
      guard.demand_requested = false;
      guard.failure = None;
      match guard.handler.take() {
        | Some(handler) => handler,
        | None => return Err(guard.port_error("finish without a handler")),
      }
    };
    let result = handler.on_upstream_finish();
    self.restore_handler(handler);
    result?;
    Ok(true)
  }
}
