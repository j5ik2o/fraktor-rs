use alloc::{boxed::Box, vec::Vec};

use super::{AsyncCallback, StageActor, StageActorReceive, TimerGraphStageLogic};
use crate::r#impl::StreamError;

/// Context passed to stage logic.
pub trait StageContext<In, Out> {
  /// Requests demand from upstream.
  fn pull(&mut self);
  /// Grabs the current input element.
  fn grab(&mut self) -> In;
  /// Pushes an element downstream.
  fn push(&mut self, out: Out);
  /// Completes the stream.
  fn complete(&mut self);
  /// Fails the stream with the provided error.
  fn fail(&mut self, error: StreamError);

  /// Returns the asynchronous callback queue for this stage.
  fn async_callback(&self) -> &AsyncCallback<Out>;
  /// Returns the timer helper for this stage.
  fn timer_graph_stage_logic(&mut self) -> &mut TimerGraphStageLogic;
  /// Creates or updates the stage actor receive callback.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when no actor system is available for this stage.
  fn get_stage_actor(&mut self, receive: Box<dyn StageActorReceive>) -> Result<StageActor, StreamError>;
  /// Returns the stage actor previously created by [`StageContext::get_stage_actor`].
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the stage actor has not been initialized.
  fn stage_actor(&self) -> Result<StageActor, StreamError>;

  /// Drains asynchronous events from the callback queue.
  #[must_use]
  fn drain_async_events(&self) -> Vec<Out> {
    self.async_callback().drain()
  }

  /// Schedules a one-shot timer.
  fn schedule_once(&mut self, key: u64, delay_ticks: u64) {
    self.timer_graph_stage_logic().schedule_once(key, delay_ticks);
  }

  /// Cancels a one-shot timer.
  #[must_use]
  fn cancel_timer(&mut self, key: u64) -> bool {
    self.timer_graph_stage_logic().cancel(key)
  }

  /// Advances timer state and returns fired keys.
  #[must_use]
  fn advance_timers(&mut self) -> Vec<u64> {
    self.timer_graph_stage_logic().advance()
  }

  /// Returns true if pull has been called on the input.
  fn has_been_pulled(&self) -> bool {
    false
  }

  /// Returns true if an input element is available to grab.
  fn is_available(&self) -> bool {
    false
  }

  /// Returns true if the input port is closed.
  fn is_closed_in(&self) -> bool {
    false
  }

  /// Returns true if the output port is closed.
  fn is_closed_out(&self) -> bool {
    false
  }
}
