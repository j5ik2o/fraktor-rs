use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};
use crate::core::{DownstreamCancelAction, KillSwitchState, KillSwitchStateHandle};

pub(in crate::core) struct CoupledTerminationLogic<In> {
  pub(in crate::core) state:              KillSwitchStateHandle,
  pub(in crate::core) shutdown_requested: bool,
  pub(in crate::core) _pd:                PhantomData<fn(In)>,
}

impl<In> CoupledTerminationLogic<In>
where
  In: Send + Sync + 'static,
{
  fn observe_state(&mut self) -> Result<(), StreamError> {
    match self.state.lock().clone() {
      | KillSwitchState::Running => Ok(()),
      | KillSwitchState::Shutdown => {
        self.shutdown_requested = true;
        Ok(())
      },
      | KillSwitchState::Aborted(error) => Err(error),
    }
  }

  fn request_coupled_shutdown(&mut self) {
    let mut state = self.state.lock();
    if matches!(&*state, KillSwitchState::Running) {
      *state = KillSwitchState::Shutdown;
    }
    self.shutdown_requested = true;
  }
}

impl<In> FlowLogic for CoupledTerminationLogic<In>
where
  In: Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    self.observe_state()?;
    if self.shutdown_requested {
      return Ok(Vec::new());
    }

    let value = downcast_value::<In>(input)?;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_tick(&mut self, _tick_count: u64) -> Result<(), StreamError> {
    self.observe_state()
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.request_coupled_shutdown();
    Ok(())
  }

  fn on_downstream_cancel(&mut self) -> Result<DownstreamCancelAction, StreamError> {
    self.request_coupled_shutdown();
    Ok(DownstreamCancelAction::Propagate)
  }

  fn take_shutdown_request(&mut self) -> bool {
    let requested = self.shutdown_requested;
    self.shutdown_requested = false;
    requested
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.shutdown_requested = false;
    Ok(())
  }
}
