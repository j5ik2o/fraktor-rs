//! Actor cell children facet for actor cells.

use alloc::vec::Vec;

use crate::actor::{
  ActorCell, Pid, SuspendReason, messaging::system_message::SystemMessage, supervision::RestartStatistics,
};

#[cfg(test)]
#[path = "actor_cell_children_test.rs"]
mod tests;

impl ActorCell {
  /// Registers a child pid for supervision.
  pub fn register_child(&self, pid: Pid) {
    self.state.with_write(|state| {
      // Pekko parity: `Children.scala:initChild` installs the child into the
      // container and lazily creates the accompanying restart stats.
      state.children_state.add_child(pid);
    });
  }

  /// Removes a child pid from supervision tracking.
  ///
  /// Children still covered by a supervision watch are left in `children_state`
  /// on purpose: the parent's `handle_death_watch_notification` is the sole
  /// consumer of the state change returned by
  /// `remove_child_and_get_state_change`. Consuming it here would drop
  /// `SuspendReason::Recreation` before the `DeathWatchNotification` emitted
  /// by `notify_watchers_on_stop` reaches the parent and the restart flow
  /// would never fire.
  ///
  /// Callers that tear down a child outside the `DeathWatchNotification`
  /// pipeline (e.g. `rollback_spawn` when the spawn handshake failed before
  /// the child ever started) are expected to unwire the supervision watch
  /// via [`ActorCell::unregister_supervision_watching`] *before* invoking
  /// this method. With the supervision watch gone, `watching_contains_pid`
  /// returns `false` and the container entry is removed normally.
  pub fn unregister_child(&self, pid: &Pid) {
    self.state.with_write(|state| {
      if state.watching_contains_pid(*pid) {
        return;
      }
      let _ = state.children_state.remove_child_and_get_state_change(*pid);
    });
  }

  pub(super) fn stop_child(&self, pid: Pid) {
    // Pekko `ActorCell.stop(actor)` (Children.scala):
    //   if (childrenRefs.getByRef(actor).isDefined) {
    //     if (!childrenRefs.isTerminating) {
    //       childrenRefs = childrenRefs.shallDie(actor)
    //       actor.stop()
    //     }
    //   }
    // Skip when either the pid is not a live child or the container is already
    // terminating (`reason == Termination`), matching Pekko's guard that
    // prevents re-stopping during parent termination.
    let should_stop = self.state.with_write(|state| {
      if !state.children_state.children().contains(&pid) || state.children_state.is_terminating() {
        return false;
      }
      state.children_state.shall_die(pid);
      true
    });
    if !should_stop {
      return;
    }
    if let Err(send_error) = self.system().send_system_message(pid, SystemMessage::Stop) {
      self.system().record_send_error(Some(pid), &send_error);
    }
  }

  /// Marks `pid` as scheduled to die on the child-registry state machine
  /// (Pekko `childrenRefs.shallDie(actor)`). Exposed for
  /// [`ActorContext::stop_child`] / [`ActorContext::stop_all_children`] so
  /// that explicit child-stop requests upgrade the container to
  /// `Terminating(UserRequest)` before the `Stop` system message is dispatched.
  pub(crate) fn mark_child_dying(&self, pid: Pid) {
    self.state.with_write(|state| state.children_state.shall_die(pid));
  }

  /// Returns the current child pids supervised by this cell.
  #[must_use]
  pub fn children(&self) -> Vec<Pid> {
    self.state.with_read(|state| state.children_state.children())
  }

  /// Returns whether the child-registry container is in the `Normal` or
  /// `Empty` state (Pekko `ChildrenContainer.isNormal`).
  ///
  /// AC-H2: exposed as an observation API so that tests and supervision
  /// paths can branch on the 4-state machine without reaching into
  /// `state.children_state` directly.
  #[must_use]
  pub fn children_state_is_normal(&self) -> bool {
    self.state.with_read(|state| state.children_state.is_normal())
  }

  /// Returns whether the child-registry container is currently in the
  /// `Terminating` variant (for any [`SuspendReason`]) or in the `Terminated`
  /// variant — i.e. the parent is waiting for its children to die.
  ///
  /// Uses the fraktor-rs convenience predicate
  /// [`ChildrenContainer::is_in_terminating_variant`]; `ChildrenContainer::is_terminating`
  /// retains the narrower Pekko parity semantics (`reason == Termination`).
  #[must_use]
  pub fn children_state_is_terminating(&self) -> bool {
    self.state.with_read(|state| state.children_state.is_in_terminating_variant())
  }

  /// Recursively propagates `SystemMessage::Suspend` to every registered child.
  ///
  /// Pekko parity: `Children.scala:203-208` `suspendChildren(exceptFor)` — the
  /// parent iterates its children and asks each of them to suspend. Each child
  /// mailbox that processes the resulting `Suspend` then propagates to its own
  /// children through the same `system_invoke` path, which is how grandchildren
  /// get reached (AC-H3-T3).
  ///
  /// Failures from `send_system_message` are logged through
  /// `record_send_error` (same convention as `handle_failure` / `stop_child`).
  /// Per `ignored-return-values.md` we observe every failure: a child whose
  /// mailbox is already closed simply produces a recorded log entry, which is
  /// the Pekko-equivalent "child already dead" outcome.
  pub(crate) fn suspend_children(&self) {
    for pid in self.children() {
      if let Err(send_error) = self.system().send_system_message(pid, SystemMessage::Suspend) {
        // Pekko parity: a child that is already stopped is a benign case —
        // `send_system_message` returns `SendError::MailboxClosed` and the
        // parent continues with the remaining children.
        self.system().record_send_error(Some(pid), &send_error);
      }
    }
  }

  /// Recursively propagates `SystemMessage::Resume` to every registered child.
  ///
  /// Pekko parity: `Children.scala:210-216` `resumeChildren(cause, perp)` —
  /// Pekko passes the failing child + cause so a per-child `Resume(cause)` can
  /// target only the perpetrator. fraktor-rs does not yet carry a cause payload
  /// on `SystemMessage::Resume` (AC-H4 responsibility), so every child is
  /// resumed unconditionally, mirroring the simpler case where `perp == null`.
  pub(crate) fn resume_children(&self) {
    for pid in self.children() {
      if let Err(send_error) = self.system().send_system_message(pid, SystemMessage::Resume) {
        self.system().record_send_error(Some(pid), &send_error);
      }
    }
  }

  pub(crate) fn snapshot_child_restart_stats(&self, pid: Pid) -> Option<RestartStatistics> {
    self.state.with_read(|state| state.children_state.stats_for(pid).cloned())
  }

  pub(super) fn mark_children_for_termination(&self) -> Option<Vec<Pid>> {
    self.state.with_write(|state| {
      if state.children_state.is_terminating() {
        return Some(Vec::new());
      }
      let children = state.children_state.children();
      if children.is_empty() {
        return None;
      }
      for child in &children {
        state.children_state.shall_die(*child);
      }
      let reason_updated = state.children_state.set_children_termination_reason(SuspendReason::Termination);
      debug_assert!(reason_updated, "children_state must be Terminating after marking live children for termination",);
      Some(children)
    })
  }
}
