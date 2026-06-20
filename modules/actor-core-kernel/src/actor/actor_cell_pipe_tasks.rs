//! Actor cell pipe tasks facet for actor cells.

use alloc::format;
use core::{mem, task::Poll};

use crate::{
  actor::{
    ActorCell,
    actor_ref::ActorRef,
    context_pipe::{ContextPipeFuture, ContextPipeTask, ContextPipeTaskId},
    error::PipeSpawnError,
    messaging::{AnyMessage, system_message::SystemMessage},
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
    let task = self.take_pipe_task_for_poll(task_id);

    let Some(mut task) = task else {
      return;
    };

    match task.poll() {
      | Poll::Ready(Some(message)) => {
        let target = task.take_delivery_target();
        self.finish_pipe_task_poll(task_id, None);
        self.deliver_pipe_task_result(message, target);
      },
      | Poll::Ready(None) => {
        self.finish_pipe_task_poll(task_id, None);
      },
      | Poll::Pending => {
        let should_repoll = self.finish_pipe_task_poll(task_id, Some(task));
        if should_repoll {
          self.schedule_pipe_task_repoll(task_id);
        }
      },
    }
  }

  fn take_pipe_task_for_poll(&self, task_id: ContextPipeTaskId) -> Option<ContextPipeTask> {
    self.state.with_write(|state| {
      let Some(index) = state.pipe_tasks.iter().position(|task| task.id() == task_id) else {
        if state.polling_pipe_tasks.contains(&task_id) && !state.pending_pipe_task_wakes.contains(&task_id) {
          state.pending_pipe_task_wakes.push(task_id);
        }
        return None;
      };
      if !state.polling_pipe_tasks.contains(&task_id) {
        state.polling_pipe_tasks.push(task_id);
      }
      Some(state.pipe_tasks.swap_remove(index))
    })
  }

  fn finish_pipe_task_poll(&self, task_id: ContextPipeTaskId, task: Option<ContextPipeTask>) -> bool {
    self.state.with_write(|state| {
      state.polling_pipe_tasks.retain(|id| *id != task_id);
      let should_repoll = state.pending_pipe_task_wakes.contains(&task_id);
      state.pending_pipe_task_wakes.retain(|id| *id != task_id);
      if let Some(task) = task {
        state.pipe_tasks.push(task);
      }
      should_repoll
    })
  }

  fn schedule_pipe_task_repoll(&self, task_id: ContextPipeTaskId) {
    if let Err(send_error) = self.system().send_system_message(self.pid(), SystemMessage::PipeTask(task_id)) {
      self.system().record_send_error(Some(self.pid()), &send_error);
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
