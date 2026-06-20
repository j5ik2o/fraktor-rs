//! Actor cell death watch facet for actor cells.

use alloc::vec::Vec;
use core::mem;

use fraktor_utils_core_rs::sync::SharedAccess;

use crate::actor::{
  ActorCell, Pid, SuspendReason, WatchKind, WatchRegistrationKind,
  error::ActorError,
  messaging::{AnyMessage, system_message::SystemMessage},
};

#[cfg(test)]
#[path = "actor_cell_death_watch_test.rs"]
mod tests;

impl ActorCell {
  /// Registers `target` as a user-level watch on this cell (Pekko
  /// `DeathWatch.watching += target`).
  ///
  /// AC-H5: user-level entry point; records the `(target, WatchKind::User)`
  /// pair. Idempotent for duplicate calls. Internal supervision watches are
  /// registered separately via [`ActorCellState::register_watching`] with
  /// [`WatchKind::Supervision`].
  pub fn register_watching(&self, target: Pid) {
    self.state.with_write(|state| state.register_watching(target, WatchKind::User));
  }

  /// Removes user-level watching of `target` (Pekko `DeathWatch.unwatch`
  /// parity for the `watching` side only).
  ///
  /// Supervision-level watches registered with [`WatchKind::Supervision`] are
  /// preserved so that `finish_recreate` / `finish_terminate` keep firing.
  ///
  /// Unlike Pekko (which is single-threaded per actor and can safely clear
  /// `terminatedQueued` here), fraktor-rs leaves the dedup marker alone: a
  /// concurrent `handle_death_watch_notification` may have just pushed
  /// `target` into `terminated_queued`, and clearing it from this code path
  /// would allow a duplicate notification to drive `finish_recreate` twice.
  /// The marker is removed naturally when the in-flight notification handler
  /// finishes (see `handle_death_watch_notification`).
  pub fn unregister_watching(&self, target: Pid) {
    self.state.with_write(|state| state.unregister_watching(target, WatchKind::User));
  }

  /// Returns whether this cell has any watch registered for `target`,
  /// regardless of [`WatchKind`].
  #[must_use]
  pub fn is_watching(&self, target: Pid) -> bool {
    self.state.with_read(|state| state.watching_contains_pid(target))
  }

  /// Classifies the current **user-level** watch registration for `target`.
  ///
  /// **User watch only.** Supervision-only entries
  /// (`WatchKind::Supervision`) are treated as
  /// [`WatchRegistrationKind::None`] so that kernel-internal parent/child
  /// bookkeeping cannot spuriously trip the duplicate check in
  /// `ActorContext::watch` / `watch_with`.
  ///
  /// Equivalent to Pekko `DeathWatch.scala:104` `watching.get(actor)` viewed
  /// through the lens of `Option[Any]`:
  ///
  /// | fraktor-rs                           | Pekko                     |
  /// |--------------------------------------|---------------------------|
  /// | [`WatchRegistrationKind::None`]      | `watching.get(ref) == None` (absent) |
  /// | [`WatchRegistrationKind::Plain`]     | `watching(ref) == None`   |
  /// | [`WatchRegistrationKind::WithMessage`] | `watching(ref) == Some(_)` |
  pub(crate) fn watch_registration_kind(&self, target: Pid) -> WatchRegistrationKind {
    self.state.with_read(|state| {
      if !state.watching_contains_user(target) {
        WatchRegistrationKind::None
      } else if state.watch_with_messages.iter().any(|(pid, _)| *pid == target) {
        WatchRegistrationKind::WithMessage
      } else {
        WatchRegistrationKind::Plain
      }
    })
  }

  /// Returns a snapshot of the `terminated_queued` set (Pekko
  /// `terminatedQueued.toSeq`).
  ///
  /// AC-H5: exposed so tests can observe dedup behaviour for
  /// `DeathWatchNotification` delivery.
  #[must_use]
  pub fn terminated_queued(&self) -> Vec<Pid> {
    self.state.with_read(|state| state.terminated_queued.clone())
  }

  /// Registers `parent_pid` as a supervision-kind watcher on this cell.
  ///
  /// Used by `spawn_with_parent` to wire the bidirectional supervision watch
  /// so that when this cell stops, `notify_watchers_on_stop` delivers a
  /// `DeathWatchNotification` to the parent (driving `finish_recreate` /
  /// `finish_terminate`). Idempotent for duplicate calls.
  pub(crate) fn register_supervision_watcher(&self, parent_pid: Pid) {
    self.state.with_write(|state| state.register_watcher(parent_pid, WatchKind::Supervision));
  }

  /// Registers `child_pid` in this cell's `watching` set with
  /// [`WatchKind::Supervision`].
  pub(crate) fn register_supervision_watching(&self, child_pid: Pid) {
    self.state.with_write(|state| state.register_watching(child_pid, WatchKind::Supervision));
  }

  /// Removes the `(child_pid, WatchKind::Supervision)` entry from this cell's
  /// `watching` set. User-level watches (`WatchKind::User`) are preserved.
  pub(crate) fn unregister_supervision_watching(&self, child_pid: Pid) {
    self.state.with_write(|state| state.unregister_watching(child_pid, WatchKind::Supervision));
  }

  pub(crate) fn handle_watch(&self, watcher: Pid) {
    let notify_immediately = self.state.with_write(|state| {
      if self.is_terminated() {
        return true;
      }
      state.register_watcher(watcher, WatchKind::User);
      false
    });
    if notify_immediately
      && let Err(send_error) =
        self.system().send_system_message(watcher, SystemMessage::DeathWatchNotification(self.pid))
    {
      self.system().record_send_error(Some(watcher), &send_error);
    }
  }

  pub(crate) fn handle_unwatch(&self, watcher: Pid) {
    self.state.with_write(|state| state.unregister_watcher(watcher, WatchKind::User));
  }

  pub(super) fn drop_watch_with_messages(&self) {
    self.state.with_write(|state| state.watch_with_messages.clear());
  }

  pub(super) fn notify_watchers_on_stop(&self) {
    let Some(recipients) = self.state.with_write(|state| {
      if state.watchers.is_empty() {
        return None;
      }
      Some(mem::take(&mut state.watchers))
    }) else {
      return;
    };

    for (watcher, _kind) in recipients {
      if let Err(send_error) =
        self.system().send_system_message(watcher, SystemMessage::DeathWatchNotification(self.pid))
      {
        self.system().record_send_error(Some(watcher), &send_error);
      }
    }
  }

  /// Handles a `SystemMessage::DeathWatchNotification(pid)` for a watched
  /// target (Pekko `DeathWatch.scala:watchedActorTerminated` +
  /// `FaultHandling.scala:handleChildTerminated`).
  ///
  /// Dispatches in the following order:
  ///
  /// 1. Drop the notification silently when `pid` is not in `watching` for any [`WatchKind`].
  /// 2. Drop the notification silently when `pid` is already in `terminated_queued` (dedup).
  /// 3. Atomically remove every `(pid, _)` entry from `watching` and push `pid` into
  ///    `terminated_queued`.
  /// 4. Consume the child-container state transition via
  ///    [`ChildrenContainer::remove_child_and_get_state_change`].
  /// 5. When a [`WatchKind::User`] entry was present, deliver either the custom `watch_with`
  ///    message (via the user mailbox) or call [`Actor::on_terminated`] directly. If only a
  ///    [`WatchKind::Supervision`] entry existed (user revoked their watch via `unwatch` but the
  ///    kernel keeps an internal supervision watch), skip user-facing dispatch and clean up any
  ///    leftover `watch_with` registration.
  /// 6. Remove `pid` from `terminated_queued` so subsequent notifications for a re-registered pid
  ///    can fire again.
  /// 7. When the state transition reported `Some(SuspendReason::Recreation(cause))`, drive
  ///    `finish_recreate` with the cause. When it reported `Some(SuspendReason::Termination)`,
  ///    drive `finish_terminate`.
  pub(crate) fn handle_death_watch_notification(&self, pid: Pid) -> Result<(), ActorError> {
    let Some((has_user_watch, state_change)) = self.state.with_write(|state| {
      if !state.watching_contains_pid(pid) {
        return None;
      }
      if state.terminated_queued.contains(&pid) {
        return None;
      }
      let has_user = state.watching.iter().any(|(existing, kind)| *existing == pid && *kind == WatchKind::User);
      state.watching.retain(|(existing, _)| *existing != pid);
      state.terminated_queued.push(pid);
      Some((has_user, state.children_state.remove_child_and_get_state_change(pid)))
    }) else {
      return Ok(());
    };

    let delivery_result = if has_user_watch {
      let custom_message = self.take_watch_with_message(pid);
      if let Some(message) = custom_message {
        self.actor_ref().try_tell(message).map_err(|error| ActorError::from_send_error(&error))
      } else {
        let mut ctx = self.make_context();
        let result = self.actor.with_write(|actor| actor.on_terminated(&mut ctx, pid));
        ctx.clear_sender();
        result
      }
    } else {
      // Supervision-only observation: user revoked their watch, so no user-facing
      // callback fires. Drop any residual `watch_with` registration for hygiene.
      self.remove_watch_with(pid);
      Ok(())
    };

    self.state.with_write(|state| state.terminated_queued.retain(|existing| *existing != pid));

    if let Some(state_change) = state_change {
      let completion_result = match state_change {
        | SuspendReason::Recreation(cause) => {
          debug_assert!(
            self.state.with_read(|state| state.deferred_recreate_cause.as_ref().is_none_or(|stored| stored == &cause)),
            "deferred_recreate_cause must match the cause returned by remove_child_and_get_state_change",
          );
          self.finish_recreate(&cause)
        },
        | SuspendReason::Termination => self.finish_terminate(),
        | SuspendReason::UserRequest => Ok(()),
      };
      // Pekko parity: user-callback delivery (on_terminated / try_tell) and
      // lifecycle completion are logically independent. If both fail, surface
      // the user-visible delivery failure.
      return match delivery_result {
        | Ok(()) => completion_result,
        | Err(delivery_error) => Err(delivery_error),
      };
    }

    delivery_result
  }

  /// Registers a custom message to deliver when the watched target terminates.
  ///
  /// **Invariant**: `ActorContext::watch_with` performs a
  /// [`watch_registration_kind`](Self::watch_registration_kind) check **before**
  /// invoking this helper, so arriving with an existing entry for `target`
  /// indicates a violation of the duplicate-check contract
  /// (`pekko-death-watch-duplicate-check` Decision 4). In debug builds this
  /// panics; in release builds the existing entry is replaced to preserve
  /// safety but the bug should be fixed upstream.
  pub(crate) fn register_watch_with(&self, target: Pid, message: AnyMessage) {
    self.state.with_write(|state| {
      debug_assert!(
        !state.watch_with_messages.iter().any(|(pid, _)| *pid == target),
        "register_watch_with invariant violated: duplicate entry for {target:?}. \
         ActorContext::watch_with must call watch_registration_kind first.",
      );
      state.watch_with_messages.retain(|(pid, _)| *pid != target);
      state.watch_with_messages.push((target, message));
    });
  }

  /// Removes any custom watch-with message for the given target.
  pub(crate) fn remove_watch_with(&self, target: Pid) {
    self.state.with_write(|state| state.watch_with_messages.retain(|(pid, _)| *pid != target));
  }

  pub(super) fn take_watch_with_message(&self, target: Pid) -> Option<AnyMessage> {
    self.state.with_write(|state| {
      if let Some(index) = state.watch_with_messages.iter().position(|(pid, _)| *pid == target) {
        let (_, message) = state.watch_with_messages.swap_remove(index);
        Some(message)
      } else {
        None
      }
    })
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub(crate) fn watchers_snapshot(&self) -> Vec<Pid> {
    self.state.with_read(|state| state.watchers.iter().map(|(pid, _)| *pid).collect())
  }
}
