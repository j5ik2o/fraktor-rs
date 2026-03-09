use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use fraktor_utils_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};
use crate::core::FailureAction;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in crate::core::stage::flow) struct LogObservation {
  pub(in crate::core::stage::flow) element_count: usize,
  pub(in crate::core::stage::flow) completed:     bool,
  pub(in crate::core::stage::flow) failure_count: usize,
}

#[derive(Clone)]
pub(in crate::core::stage::flow) struct LogObservationHandle {
  inner: ArcShared<SpinSyncMutex<LogObservation>>,
}

impl LogObservationHandle {
  pub(in crate::core::stage::flow) fn new() -> Self {
    Self { inner: ArcShared::new(SpinSyncMutex::new(LogObservation::default())) }
  }

  pub(in crate::core::stage::flow) fn record_element(&self) {
    let mut guard = self.inner.lock();
    guard.element_count = guard.element_count.saturating_add(1);
  }

  pub(in crate::core::stage::flow) fn record_completion(&self) {
    self.inner.lock().completed = true;
  }

  pub(in crate::core::stage::flow) fn record_failure(&self) {
    let mut guard = self.inner.lock();
    guard.failure_count = guard.failure_count.saturating_add(1);
  }
}

impl Default for LogObservationHandle {
  fn default() -> Self {
    Self::new()
  }
}

pub(in crate::core::stage::flow) struct LogLogic<In> {
  pub(in crate::core::stage::flow) observation: LogObservationHandle,
  pub(in crate::core::stage::flow) _pd:         PhantomData<fn(In)>,
}

impl<In> LogLogic<In> {
  pub(in crate::core::stage::flow) fn new(observation: LogObservationHandle) -> Self {
    Self { observation, _pd: PhantomData }
  }
}

impl<In> FlowLogic for LogLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.observation.record_element();
    Ok(vec![Box::new(value) as DynValue])
  }

  fn handles_failures(&self) -> bool {
    false
  }

  fn on_failure(&mut self, error: StreamError) -> Result<FailureAction, StreamError> {
    self.observation.record_failure();
    Ok(FailureAction::Propagate(error))
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.observation.record_completion();
    Ok(())
  }
}
