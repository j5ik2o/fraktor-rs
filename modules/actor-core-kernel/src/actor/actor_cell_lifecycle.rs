//! Actor cell lifecycle facet for actor cells.

use fraktor_utils_core_rs::sync::SharedAccess;
use portable_atomic::Ordering;

use crate::{
  actor::{
    ActorCell,
    error::ActorError,
    lifecycle::{LifecycleEvent, LifecycleStage},
    messaging::system_message::SystemMessage,
  },
  event::stream::EventStreamEvent,
  system::guardian::GuardianKind,
};

impl ActorCell {
  pub(super) fn mark_terminated(&self) {
    self.terminated.store(true, Ordering::Release);
    self.drop_adapter_refs();
    self.drop_pipe_tasks();
  }

  pub(super) fn is_terminated(&self) -> bool {
    self.terminated.load(Ordering::Acquire)
  }

  pub(super) fn handle_create(&self) -> Result<(), ActorError> {
    let outcome = self.run_pre_start(LifecycleStage::Started);
    if let Err(ref error) = outcome {
      self.report_failure(error, None);
    }
    outcome
  }

  pub(super) fn handle_stop(&self) -> Result<(), ActorError> {
    {
      let mut ctx = self.make_context();
      ctx.cancel_receive_timeout();
      ctx.clear_sender();
    }

    if let Some(children) = self.mark_children_for_termination() {
      if children.is_empty() {
        return Ok(());
      }
      self.mailbox().suspend();
      let dispatcher = self.new_dispatcher_shared();
      dispatcher.run_with_drive_guard(|| {
        for child in &children {
          if let Err(send_error) = self.system().send_system_message(*child, SystemMessage::Stop) {
            self.system().record_send_error(Some(*child), &send_error);
          }
        }
      });
      return Ok(());
    }

    self.finish_terminate()
  }

  pub(super) fn finish_terminate(&self) -> Result<(), ActorError> {
    debug_assert!(
      self.children().is_empty(),
      "finish_terminate expects all children to be removed from children_state"
    );

    let mut ctx = self.make_context();
    ctx.cancel_receive_timeout();
    let result = self.actor.with_write(|actor| actor.post_stop(&mut ctx));
    ctx.clear_sender();
    if result.is_ok() {
      self.publish_lifecycle(LifecycleStage::Stopped);
    }

    self.drop_stash_messages();
    self.drop_timer_handles();
    self.mark_terminated();
    self.notify_watchers_on_stop();

    if let Some(parent) = self.parent {
      self.system().unregister_child(Some(parent), self.pid);
    }

    self.system().release_name(self.parent, &self.name);
    self.system().remove_cell(&self.pid);

    if let Some(kind) = self.system().guardian_kind_by_pid(self.pid) {
      self.system().mark_guardian_stopped(kind);
      match kind {
        | GuardianKind::Root => {
          self.system().mark_terminated();
        },
        | GuardianKind::User | GuardianKind::System => {
          if !self.system().guardian_alive(GuardianKind::Root) {
            self.system().mark_terminated();
          }
        },
      }
    }

    result
  }

  fn run_pre_start(&self, stage: LifecycleStage) -> Result<(), ActorError> {
    let mut ctx = self.make_context();
    let outcome = self.actor.with_write(|actor| actor.pre_start(&mut ctx));
    ctx.clear_sender();
    if outcome.is_ok() {
      self.publish_lifecycle(stage);
    }
    outcome
  }

  pub(super) fn publish_lifecycle(&self, stage: LifecycleStage) {
    let timestamp = self.system().monotonic_now();
    let event = LifecycleEvent::new(self.pid, self.parent, self.name.clone(), stage, timestamp);
    self.system().publish_event(&EventStreamEvent::Lifecycle(event));
  }
}
