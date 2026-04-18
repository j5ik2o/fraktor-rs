use alloc::{boxed::Box, vec, vec::Vec};

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

/// Keep-alive flow logic.
///
/// Mirrors `KeepAlive` in Apache Pekko (`Flow.scala:3080`).
/// When no element arrives for `max_idle_ticks` ticks, the `injected_elem` is pushed
/// downstream to keep the connection alive.  Normal elements pass through unchanged
/// and reset the idle timer.
pub(in crate::core) struct KeepAliveLogic<In> {
  // アイドルタイムアウトの tick 数
  pub(in crate::core) max_idle_ticks:   u64,
  // 最後に要素を受信または注入した tick
  pub(in crate::core) last_active_tick: u64,
  // 現在の tick カウント
  pub(in crate::core) tick_count:       u64,
  // アイドル時に注入する要素
  pub(in crate::core) injected_elem:    In,
  // ドレイン待ちの注入要素（tick で生成された pending output）
  pub(in crate::core) pending_injected: Option<In>,
}

impl<In> FlowLogic for KeepAliveLogic<In>
where
  In: Clone + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    // 要素が来たらアイドルタイマーをリセットする
    self.last_active_tick = self.tick_count;
    // pending があれば捨てる（新要素が来たのでキープアライブ不要）
    self.pending_injected = None;
    Ok(vec![Box::new(value) as DynValue])
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    // アイドル期間がしきい値を超えたら注入要素を pending にセット
    if self.tick_count.saturating_sub(self.last_active_tick) >= self.max_idle_ticks && self.pending_injected.is_none() {
      self.pending_injected = Some(self.injected_elem.clone());
      // 注入したらタイマーをリセット（次のアイドルまで再注入しない）
      self.last_active_tick = self.tick_count;
    }
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if let Some(value) = self.pending_injected.take() {
      return Ok(vec![Box::new(value) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn has_pending_output(&self) -> bool {
    self.pending_injected.is_some()
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.tick_count = 0;
    self.last_active_tick = 0;
    self.pending_injected = None;
    Ok(())
  }
}
