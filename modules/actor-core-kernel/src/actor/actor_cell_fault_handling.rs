//! Actor cell fault handling facet for actor cells.

use alloc::{vec, vec::Vec};
use core::time::Duration;

use fraktor_utils_core_rs::sync::SharedAccess;

use crate::{
  actor::{
    ActorCell, FailedInfo, Pid, SuspendReason,
    error::{ActorError, ActorErrorReason},
    lifecycle::LifecycleStage,
    messaging::system_message::{FailureMessageSnapshot, FailurePayload, SystemMessage},
    supervision::{SupervisorDirective, SupervisorStrategyKind},
  },
  system::state::system_state::FailureOutcome,
};

impl ActorCell {
  /// Returns whether the cell currently has a failure recorded (Pekko
  /// `isFailed`).
  ///
  /// AC-H3 extension: both `FailedRef(perpetrator)` and `FailedFatally`
  /// count as failed; only `NoFailedInfo` returns `false`.
  #[must_use]
  pub fn is_failed(&self) -> bool {
    self.state.with_read(|state| matches!(state.failed, FailedInfo::Child(_) | FailedInfo::Fatal))
  }

  /// Returns whether the cell is currently in the `FailedFatally` state
  /// (Pekko `isFailedFatally`).
  ///
  /// AC-H3 extension: a fatal failure prevents any further restart attempt
  /// until `clear_failed` is called (e.g. through `finishCreate` /
  /// `finishRecreate`).
  #[must_use]
  pub fn is_failed_fatally(&self) -> bool {
    self.state.with_read(|state| matches!(state.failed, FailedInfo::Fatal))
  }

  /// Returns the [`Pid`] of the child whose failure is currently being
  /// processed, if any (Pekko `perpetrator`).
  ///
  /// AC-H3 extension: only the `FailedRef(perpetrator)` state yields a pid;
  /// `NoFailedInfo` and `FailedFatally` both return `None`.
  #[must_use]
  pub fn perpetrator(&self) -> Option<Pid> {
    self.state.with_read(|state| match state.failed {
      | FailedInfo::Child(pid) => Some(pid),
      | FailedInfo::None | FailedInfo::Fatal => None,
    })
  }

  /// Records a failure with `perpetrator` unless the cell is already in the
  /// `FailedFatally` state (Pekko `setFailed`).
  ///
  /// AC-H3 extension: fatal failures take priority and are never downgraded
  /// to a `FailedRef` by a subsequent child failure.
  pub fn set_failed(&self, perpetrator: Pid) {
    self.state.with_write(|state| {
      // Pekko parity (`FaultHandling.scala`): `setFailed` guards against
      // overwriting `FailedFatally`, so a later child failure cannot downgrade
      // an already-fatal state.
      if matches!(state.failed, FailedInfo::Fatal) {
        return;
      }
      state.failed = FailedInfo::Child(perpetrator);
    });
  }

  /// Marks the cell as fatally failed (Pekko `setFailedFatally`).
  ///
  /// AC-H3 extension: unconditionally overwrites any prior `FailedRef` state
  /// with `FailedFatally` so that subsequent `set_failed` calls are ignored.
  pub fn set_failed_fatally(&self) {
    self.state.with_write(|state| {
      state.failed = FailedInfo::Fatal;
    });
  }

  /// Clears any recorded failure state (Pekko `clearFailed`).
  ///
  /// AC-H3 extension: unconditionally resets the cell to `NoFailedInfo`,
  /// including the `FailedFatally` state — required by the `finishCreate` /
  /// `finishRecreate` restart completion path.
  pub fn clear_failed(&self) {
    self.state.with_write(|state| {
      state.failed = FailedInfo::None;
    });
  }

  /// Pekko `FaultHandling.scala:92-118` faultRecreate: drives the first phase
  /// of the restart state machine.
  ///
  /// The method calls `pre_restart(&mut ctx, &cause)` and either falls through
  /// to `finish_recreate(cause)` immediately (no live children) or defers the
  /// completion by tagging `ChildrenContainer` with
  /// `SuspendReason::Recreation(cause)` and storing the cause in
  /// `deferred_recreate_cause` until the last child terminates.
  pub(crate) fn fault_recreate(&self, cause: &ActorErrorReason) -> Result<(), ActorError> {
    // Pekko parity: when the cell is already marked as fatally failed, Pekko
    // keeps the actor `null` and treats `faultRecreate` as a no-op. fraktor-rs
    // preserves the same semantics so subsequent callbacks do not fire.
    if self.is_failed_fatally() {
      return Ok(());
    }

    {
      let mut ctx = self.make_context();
      ctx.cancel_receive_timeout();
      // Pekko parity under sync dispatch: default `pre_restart` invokes
      // `stop_all_children`, which sends `SystemMessage::Stop` to each child.
      // When the outer invocation is driven via `ActorCellInvoker::system_invoke`
      // (e.g. test direct calls) the executor's `running` flag is not yet held,
      // so the first nested `execute` for the child mailbox would claim the
      // drain-owner slot and drain on the same thread — reentering into the
      // parent before `set_children_termination_reason(Recreation)` runs.
      // `run_with_drive_guard` claims the slot via the existing
      // `ExecutorShared` trampoline for the duration of `pre_restart`, forcing
      // child mailbox work to queue up instead. Production dispatchers already
      // enter the trampoline when `mailbox.run` is scheduled on a worker
      // thread, so this wrap is effectively a no-op there.
      // 範囲制限: この guard が保護するのは親と同一の `ExecutorShared`（=同一 dispatcher）
      // 配下の child のみ。`with_dispatcher_id` で別 dispatcher を割り当てた child の
      // `send_system_message` → その dispatcher 側の `system_dispatch` は親とは別の
      // trampoline を通るため、guard の外で実行され得る。クロス dispatcher 下の
      // 再入防止は各 dispatcher 側の CAS ベース drain-owner 選択が担う。
      let dispatcher = self.new_dispatcher_shared();
      let pre_restart_result =
        dispatcher.run_with_drive_guard(|| self.actor.with_write(|actor| actor.pre_restart(&mut ctx, cause)));
      pre_restart_result?;
      ctx.clear_sender();
    }

    debug_assert!(
      self.mailbox().is_suspended(),
      "fault_recreate expects the mailbox to be suspended (AC-H3 precondition)"
    );

    let deferred = self.state.with_write(|state| {
      state.deferred_recreate_cause = Some(cause.clone());
      state.children_state.set_children_termination_reason(SuspendReason::Recreation(cause.clone()))
    });

    if deferred {
      // `finish_recreate` will fire from `handle_death_watch_notification` once
      // the last live child terminates.
      return Ok(());
    }

    self.finish_recreate(cause)
  }

  /// Pekko `FaultHandling.scala:278-303` finishRecreate: second phase of the
  /// restart state machine. Performs the actual actor recreation and drives
  /// `post_restart`.
  pub(crate) fn finish_recreate(&self, cause: &ActorErrorReason) -> Result<(), ActorError> {
    self.state.with_write(|state| {
      state.deferred_recreate_cause.take();
      // Pekko `FaultHandling.scala:294` parity: at this point
      // `children_state` must no longer be Terminating. Two paths reach
      // finish_recreate:
      //   1. Immediate path from fault_recreate when `set_children_termination_reason` returned false —
      //      the container was Normal/Empty to begin with.
      //   2. Deferred path from handle_death_watch_notification — `remove_child_and_get_state_change`
      //      transitions the container out of Terminating once the last `to_die` child dies.
      // Assert the invariant so a future regression surfaces early.
      debug_assert!(
        !state.children_state.is_in_terminating_variant(),
        "finish_recreate expects children_state to be Normal/Empty/Terminated, not Terminating"
      );
    });

    self.drop_pipe_tasks();
    self.drop_stash_messages();
    self.drop_timer_handles();
    self.drop_watch_with_messages();
    self.publish_lifecycle(LifecycleStage::Stopped);
    self.recreate_actor();
    // Pekko `FaultHandling.scala:173` `finishCreate` / `:284` `finishRecreate`:
    //   try resumeNonRecursive() finally clearFailed()
    // Clears `FailedInfo` (set by `report_failure` via AC-M3's
    // `set_failed(self.pid)` wiring) so the fresh actor instance starts
    // from `FailedInfo::None`. Paired with `SystemMessage::Resume` arm
    // to cover both Restart and Resume supervisor directives.
    self.clear_failed();

    let outcome = {
      let mut ctx = self.make_context();
      let result = self.actor.with_write(|actor| actor.post_restart(&mut ctx, cause));
      ctx.clear_sender();
      result
    };
    match outcome {
      | Ok(()) => {
        // Pekko `FaultHandling.scala:292` と同様に `post_restart` 成功後に mailbox を
        // resume する。先に resume してしまうと、dispatcher 実装によっては再初期化前の
        // actor に user message が配送される可能性がある。
        self.mailbox().resume();
        self.publish_lifecycle(LifecycleStage::Restarted);
        Ok(())
      },
      | Err(error) => {
        // fault_recreate の AC-H3 precondition により mailbox は既に suspended。
        // report_failure は supervisor へ報告する前に mailbox.suspend() を呼ぶため、
        // ここで先に resume して suspend_count を入口時点の値に戻しておかないと、
        // カウンタが二重に増え、supervisor からの単発 Resume で mailbox が再開
        // できず永続的に stuck する。
        self.mailbox().resume();
        self.set_failed_fatally();
        self.report_failure(&error, None);
        Err(error)
      },
    }
  }

  pub(super) fn handle_kill(&self, snapshot: Option<FailureMessageSnapshot>) -> Result<(), ActorError> {
    let error = ActorError::fatal("Kill");
    self.report_failure(&error, snapshot);
    Err(error)
  }

  /// Reports an invocation failure to the supervisor, following Pekko
  /// `FaultHandling.scala:215-234` `handleInvokeFailure` step-by-step:
  ///
  /// 1. `suspendNonRecursive()` (L218) — suspend this actor's mailbox.
  /// 2. `case _ if !isFailed => setFailed(self)` (L222, AC-M3) — record the perpetrator as
  ///    `self.pid` when not already failed. The `is_failed()` guard prevents overwriting a prior
  ///    perpetrator on duplicate reports, and the inner `set_failed` implementation
  ///    (`actor_cell.rs:448`) additionally preserves `FailedInfo::Fatal` against downgrade — the
  ///    two guards compose so that neither existing `Child(_)` nor `Fatal` state is disturbed.
  /// 3. `suspendChildren(...)` (L225, AC-H3) — recursively suspend children.
  /// 4. `sendSystemMessage(Failed(...))` (L231-234) — hand the failure to the supervisor through
  ///    `system.report_failure(payload)`. This always fires (independent of the `isFailed` guard)
  ///    so Pekko's "report on every occurrence" semantics is preserved.
  ///
  /// The AC-H3 extension requires the parent mailbox and every descendant
  /// to be suspended prior to `system.report_failure` so the supervisor
  /// directive sees a fully quiesced subtree.
  pub(super) fn report_failure(&self, error: &ActorError, snapshot: Option<FailureMessageSnapshot>) {
    // Pekko `FaultHandling.scala:218` suspendNonRecursive()
    self.mailbox().suspend();
    // Pekko `FaultHandling.scala:221-222` handleInvokeFailure:
    //   case _ if !isFailed => setFailed(self); Set.empty
    // fraktor-rs の report_failure は user / system message 処理失敗で
    // 呼ばれる self-failure 経路のため、perpetrator は常に self.pid。
    // child perpetrator 分岐 (Pekko L221) は現行 `FailureMessageSnapshot`
    // に child pid 情報が含まれないため AC-M3 のスコープ外 (Decision 3)。
    // is_failed() guard が既存 perpetrator (Child(_) もしくは Fatal) を
    // overwrite しないことを保証する。
    if !self.is_failed() {
      self.set_failed(self.pid);
    }
    // Pekko `FaultHandling.scala:225` suspendChildren(exceptFor = skip)
    // self-failure 経路のため skip = empty (全子を suspend)。
    self.suspend_children();
    let timestamp = self.system().monotonic_now();
    let payload = FailurePayload::from_error(self.pid, error, snapshot, timestamp);
    // Pekko `FaultHandling.scala:231-234` parent.sendSystemMessage(Failed(...))
    // guard 通過有無に関わらず毎回 supervisor へ通知する (Pekko 同挙動)。
    self.system().report_failure(payload);
  }

  /// Processes a child failure, mirroring Pekko `FaultHandling.scala:305`
  /// `handleFailure(f: Failed)`: runs the supervisor decision, notifies the
  /// actor, and applies the directive.
  pub(super) fn handle_failure(&self, payload: &FailurePayload) {
    let actor_error = payload.to_actor_error();
    let now = self.system().monotonic_now();
    let payload_ref = &payload;
    let (directive, affected) = self.handle_child_failure(payload.child(), &actor_error, now);

    {
      let mut ctx = self.make_context();
      if let Err(ref error) =
        self.actor.with_write(|actor| actor.on_child_failed(&mut ctx, payload.child(), &actor_error))
      {
        self.report_failure(error, None);
      }
      ctx.clear_sender();
    }

    match directive {
      | SupervisorDirective::Restart => {
        // Pekko `SupervisorStrategy.restartChild(..., suspendFirst)`: the
        // originally failing child is already suspended (via its own
        // `report_failure`), but AllForOne siblings must be suspended before
        // `Recreate` arrives so `fault_recreate` observes the AC-H3
        // "suspended mailbox" precondition.
        for target in &affected {
          if *target != payload.child()
            && let Some(sibling_cell) = self.system().cell(target)
          {
            sibling_cell.mailbox().suspend();
            sibling_cell.suspend_children();
          }
        }
        let mut restart_failed = false;
        for target in affected {
          let cause = actor_error.to_reason();
          if let Err(send_error) = self.system().send_system_message(target, SystemMessage::Recreate(cause)) {
            self.system().record_send_error(Some(target), &send_error);
            restart_failed = true;
          }
        }

        if restart_failed {
          self.system().record_failure_outcome(payload.child(), FailureOutcome::Escalate, payload_ref);
          let snapshot = payload.message().cloned();
          // Pekko `FaultHandling.scala:62-67` handleInvokeFailure: the
          // supervisor itself is now failing (could not restart the child),
          // so it must suspend its own mailbox + children before reporting
          // upward. `report_failure` centralises that sequence.
          self.report_failure(&actor_error, snapshot);
        } else {
          self.system().record_failure_outcome(payload.child(), FailureOutcome::Restart, payload_ref);
        }
      },
      | SupervisorDirective::Stop => {
        for target in affected {
          if let Err(send_error) = self.system().send_system_message(target, SystemMessage::Stop) {
            self.system().record_send_error(Some(target), &send_error);
          }
        }
        self.system().record_failure_outcome(payload.child(), FailureOutcome::Stop, payload_ref);
      },
      | SupervisorDirective::Escalate => {
        for target in affected {
          if let Err(send_error) = self.system().send_system_message(target, SystemMessage::Stop) {
            self.system().record_send_error(Some(target), &send_error);
          }
        }
        self.system().record_failure_outcome(payload.child(), FailureOutcome::Escalate, payload_ref);
        let snapshot = payload.message().cloned();
        // Pekko `FaultHandling.scala:62-67` handleInvokeFailure semantics:
        // escalation from this supervisor means it will itself become the
        // subject of a restart decision by its own parent. Suspend the
        // mailbox + children so the grandparent-issued `Recreate` finds the
        // cell in the AC-H3 precondition state for `fault_recreate`.
        self.report_failure(&actor_error, snapshot);
      },
      | SupervisorDirective::Resume => {
        for target in affected {
          if let Err(send_error) = self.system().send_system_message(target, SystemMessage::Resume) {
            self.system().record_send_error(Some(target), &send_error);
          }
        }
        self.system().record_failure_outcome(payload.child(), FailureOutcome::Resume, payload_ref);
      },
    }
  }

  pub(crate) fn handle_child_failure(
    &self,
    child: Pid,
    error: &ActorError,
    now: Duration,
  ) -> (SupervisorDirective, Vec<Pid>) {
    // Get supervisor strategy dynamically from actor instance
    let strategy = {
      let mut ctx = self.make_context();
      self.actor.with_read(|actor| actor.supervisor_strategy(&mut ctx))
    };

    let directive = {
      self.state.with_write(|state| {
        // Pekko parity (`Children.scala:handleChildTerminated`): obtain the
        // restart stats from the container, creating a fresh entry if the
        // child had not been seen yet. `stats_for_mut` returns `None` only on
        // the `Terminated` state, which is unreachable here — a failed child
        // necessarily means the parent is still alive.
        match state.children_state.stats_for_mut(child) {
          | Some(entry) => strategy.handle_failure(entry, error, now),
          | None => {
            // Defensive fallback: if we are somehow in a `Terminated` state,
            // short-circuit to `Stop` to avoid restarting a dead container.
            SupervisorDirective::Stop
          },
        }
      })
    };

    let affected = match strategy.kind() {
      | SupervisorStrategyKind::OneForOne => vec![child],
      | SupervisorStrategyKind::AllForOne => self.state.with_read(|state| state.children_state.children()),
    };

    if matches!(directive, SupervisorDirective::Stop) {
      self.clear_child_stats(&affected);
    }

    (directive, affected)
  }

  fn clear_child_stats(&self, children: &[Pid]) {
    if children.is_empty() {
      return;
    }
    self.state.with_write(|state| {
      // Pekko parity: when the strategy directive is `Stop`, affected children
      // are removed from the container. We drop the returned state-change
      // reasons — AC-H4 will consume them to drive `finishRecreate` /
      // `finishTerminate`.
      for pid in children {
        let _ = state.children_state.remove_child_and_get_state_change(*pid);
      }
    });
  }
}
