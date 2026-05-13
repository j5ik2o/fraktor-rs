use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};
use crate::{DownstreamCancelAction, KillSwitchStateHandle, KillSwitchStatus};

pub(crate) struct CoupledTerminationLogic<In> {
  pub(crate) state:              KillSwitchStateHandle,
  pub(crate) shutdown_requested: bool,
  pub(crate) _pd:                PhantomData<fn(In)>,
}

impl<In> CoupledTerminationLogic<In>
where
  In: Send + Sync + 'static,
{
  fn observe_state(&mut self) -> Result<(), StreamError> {
    match self.state.lock().status().clone() {
      | KillSwitchStatus::Running => Ok(()),
      | KillSwitchStatus::Shutdown => {
        self.shutdown_requested = true;
        Ok(())
      },
      | KillSwitchStatus::Aborted(error) => Err(error),
    }
  }

  fn request_coupled_shutdown(&mut self) {
    let command_targets = {
      let mut state = self.state.lock();
      state.request_shutdown()
    };
    if let Some(command_targets) = command_targets {
      for target in command_targets {
        if target.shutdown().is_err() {
          // Actor command delivery is best-effort because the FlowLogic contract
          // has no error channel for source-done initiated coupled shutdown.
        }
      }
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
