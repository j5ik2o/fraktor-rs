#[cfg(test)]
mod tests;

use core::marker::PhantomData;

use super::{GraphStageLogic, StageContext};
use crate::core::{KillSwitchStateHandle, KillSwitchStatus, StreamError, UniqueKillSwitch};

/// Stage logic mixin that intercepts callbacks based on a kill-switch state.
///
/// Mirrors Apache Pekko's `pekko.stream.stage.KillableGraphStageLogic`: wraps
/// an inner [`GraphStageLogic`] and, on every callback, first checks the
/// shared [`KillSwitchState`]. When the switch is still `Running`, the
/// callback is forwarded verbatim to the inner logic. Once the switch
/// transitions to `Shutdown`, the next callback completes the stream via
/// [`StageContext::complete`] without driving the inner logic. When the
/// switch transitions to `Aborted(error)`, the next callback fails the
/// stream via [`StageContext::fail`] with the original error.
///
/// The `materialized()` value is always produced by the inner logic and is
/// independent of the kill-switch state.
pub struct KillableGraphStageLogic<L, In, Out, Mat> {
  inner: L,
  state: KillSwitchStateHandle,
  _pd:   PhantomData<fn(In, Out) -> Mat>,
}

impl<L, In, Out, Mat> KillableGraphStageLogic<L, In, Out, Mat> {
  /// Creates a new killable logic from an inner logic and a shared state handle.
  ///
  /// This constructor is crate-internal because `KillSwitchStateHandle` is not
  /// part of the public API. External callers should use
  /// [`KillableGraphStageLogic::from_kill_switch`] instead.
  pub(in crate::core) fn new(inner: L, state: KillSwitchStateHandle) -> Self {
    Self { inner, state, _pd: PhantomData }
  }

  /// Creates a new killable logic tied to the supplied [`UniqueKillSwitch`].
  ///
  /// The returned logic shares the kill-switch state with `switch`, so any
  /// `switch.shutdown()` / `switch.abort(error)` request is observed by the
  /// logic the next time one of its callbacks fires.
  #[must_use]
  pub fn from_kill_switch(inner: L, switch: &UniqueKillSwitch) -> Self {
    Self::new(inner, switch.state_handle())
  }
}

impl<L, In, Out, Mat> KillableGraphStageLogic<L, In, Out, Mat>
where
  L: GraphStageLogic<In, Out, Mat>,
{
  /// Returns `true` when the kill switch still reports `Running`.
  ///
  /// On `Shutdown` or `Aborted`, drives the provided `ctx` accordingly
  /// (`complete` or `fail(error)`) and returns `false`, so the calling
  /// callback must short-circuit and skip the inner delegation.
  fn check_state(&mut self, ctx: &mut dyn StageContext<In, Out>) -> bool {
    let state = self.state.lock().status().clone();
    match state {
      | KillSwitchStatus::Running => true,
      | KillSwitchStatus::Shutdown => {
        ctx.complete();
        false
      },
      | KillSwitchStatus::Aborted(error) => {
        ctx.fail(error);
        false
      },
    }
  }
}

impl<L, In, Out, Mat> GraphStageLogic<In, Out, Mat> for KillableGraphStageLogic<L, In, Out, Mat>
where
  L: GraphStageLogic<In, Out, Mat>,
{
  fn on_start(&mut self, ctx: &mut dyn StageContext<In, Out>) {
    if self.check_state(ctx) {
      self.inner.on_start(ctx);
    }
  }

  fn on_pull(&mut self, ctx: &mut dyn StageContext<In, Out>) {
    if self.check_state(ctx) {
      self.inner.on_pull(ctx);
    }
  }

  fn on_push(&mut self, ctx: &mut dyn StageContext<In, Out>) {
    if self.check_state(ctx) {
      self.inner.on_push(ctx);
    }
  }

  fn on_complete(&mut self, ctx: &mut dyn StageContext<In, Out>) {
    if self.check_state(ctx) {
      self.inner.on_complete(ctx);
    }
  }

  fn on_error(&mut self, ctx: &mut dyn StageContext<In, Out>, error: StreamError) {
    if self.check_state(ctx) {
      self.inner.on_error(ctx, error);
    }
  }

  fn on_async_callback(&mut self, ctx: &mut dyn StageContext<In, Out>) {
    if self.check_state(ctx) {
      self.inner.on_async_callback(ctx);
    }
  }

  fn on_timer(&mut self, ctx: &mut dyn StageContext<In, Out>, timer_key: u64) {
    if self.check_state(ctx) {
      self.inner.on_timer(ctx, timer_key);
    }
  }

  fn on_stop(&mut self, ctx: &mut dyn StageContext<In, Out>) {
    if self.check_state(ctx) {
      self.inner.on_stop(ctx);
    }
  }

  fn materialized(&mut self) -> Mat {
    self.inner.materialized()
  }
}
