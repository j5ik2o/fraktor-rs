//! Actor cell pipe tasks facet for actor cells.

use alloc::format;
use core::{mem, task::Poll};

use crate::{
  actor::{
    ActorCell,
    actor_ref::ActorRef,
    context_pipe::{ContextPipeFuture, ContextPipeTask, ContextPipeTaskId},
    error::PipeSpawnError,
    messaging::AnyMessage,
  },
  event::logging::LogLevel,
};

#[cfg(test)]
#[path = "actor_cell_pipe_tasks_test.rs"]
mod tests;

impl ActorCell {
  /// Registers a new pipe task targeting the actor itself and schedules its first poll.
  pub(crate) fn spawn_pipe_task(&self, future: ContextPipeFuture) -> Result<(), PipeSpawnError> {
    self.spawn_pipe_task_inner(future, None)
  }

  /// Registers a new pipe task targeting an external actor and schedules its first poll.
  pub(crate) fn spawn_pipe_to_task(&self, future: ContextPipeFuture, target: ActorRef) -> Result<(), PipeSpawnError> {
    self.spawn_pipe_task_inner(future, Some(target))
  }

  fn spawn_pipe_task_inner(&self, future: ContextPipeFuture, target: Option<ActorRef>) -> Result<(), PipeSpawnError> {
    let id = self.state.with_write(|state| {
      if self.is_terminated() {
        return Err(PipeSpawnError::TargetStopped);
      }
      let id = ContextPipeTaskId::new(state.pipe_task_counter.wrapping_add(1));
      state.pipe_task_counter = id.get();
      let task = match target {
        | Some(t) => ContextPipeTask::new_with_target(id, future, self.pid, self.system(), t),
        | None => ContextPipeTask::new(id, future, self.pid, self.system()),
      };
      state.pipe_tasks.push(task);
      Ok(id)
    })?;
    self.poll_pipe_task(id);
    Ok(())
  }

  fn poll_pipe_task(&self, task_id: ContextPipeTaskId) {
    let task = self.state.with_write(|state| {
      let tasks = &mut state.pipe_tasks;
      let index = tasks.iter().position(|task| task.id() == task_id)?;
      Some(tasks.swap_remove(index))
    });

    let Some(mut task) = task else {
      return;
    };

    match task.poll() {
      | Poll::Ready(Some(message)) => {
        let target = task.take_delivery_target();
        self.deliver_pipe_task_result(message, target);
      },
      | Poll::Ready(None) => {},
      | Poll::Pending => {
        self.state.with_write(|state| state.pipe_tasks.push(task));
      },
    }
  }

  fn deliver_pipe_task_result(&self, message: AnyMessage, target: Option<ActorRef>) {
    if let Some(mut target_ref) = target {
      let target_pid = target_ref.pid();
      if let Err(send_error) = target_ref.try_tell(message) {
        self.system().record_send_error(Some(target_pid), &send_error);
        self.system().emit_log(
          LogLevel::Warn,
          format!("pipe_to delivery failed for target {:?}: {:?}", target_pid, send_error),
          Some(self.pid()),
          None,
        );
      }
    } else {
      let self_pid = self.pid();
      let mut self_ref = self.actor_ref();
      if let Err(send_error) = self_ref.try_tell(message) {
        self.system().record_send_error(Some(self_pid), &send_error);
        self.system().emit_log(
          LogLevel::Warn,
          format!("pipe_to_self delivery failed for {:?}: {:?}", self_pid, send_error),
          Some(self_pid),
          None,
        );
      }
    }
  }

  pub(super) fn drop_pipe_tasks(&self) {
    let tasks = self.state.with_write(|state| mem::take(&mut state.pipe_tasks));
    drop(tasks);
  }

  pub(super) fn handle_pipe_task_ready(&self, task_id: ContextPipeTaskId) {
    self.poll_pipe_task(task_id)
  }
}
