use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};
use crate::{KillSwitchStateHandle, KillSwitchStatus};

pub(crate) struct KillSwitchLogic<In> {
  pub(crate) state:              KillSwitchStateHandle,
  pub(crate) shutdown_requested: bool,
  pub(crate) _pd:                PhantomData<fn(In)>,
}

impl<In> KillSwitchLogic<In>
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
}

impl<In> FlowLogic for KillSwitchLogic<In>
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
