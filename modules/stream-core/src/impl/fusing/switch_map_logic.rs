use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::SecondarySourceBridge;
use crate::{DownstreamCancelAction, DynValue, FlowLogic, StreamError, downcast_value, dsl::Source};

/// Switch-map flow logic.
///
/// Mirrors `SwitchMap` in Apache Pekko (`Flow.scala:3002`).
/// When a new outer element arrives, the previously running inner `Source` is
/// **cancelled** (dropped) and only the newest inner source is consumed.
/// This contrasts with `flat_map_merge(1, …)` / `flat_map_concat`, which wait
/// for the current inner source to finish before starting the next.
pub(crate) struct SwitchMapLogic<In, Out, Mat2, F> {
  pub(crate) func:         F,
  // 現在実行中のサブストリーム（新要素到着時に上書き＝キャンセル）
  pub(crate) active_inner: Option<SecondarySourceBridge<Out>>,
  pub(crate) _pd:          PhantomData<fn(In) -> (Out, Mat2)>,
}

impl<In, Out, Mat2, F> SwitchMapLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn switch_to(&mut self, value: In) -> Result<(), StreamError> {
    // 前のサブストリームを破棄（= キャンセル）
    self.active_inner = None;
    let inner = SecondarySourceBridge::new((self.func)(value))?;
    self.active_inner = Some(inner);
    Ok(())
  }

  fn pop_next_value(&mut self) -> Result<Option<Out>, StreamError> {
    let Some(stream) = self.active_inner.as_mut() else {
      return Ok(None);
    };
    if let Some(value) = stream.poll_next()? {
      if !stream.has_pending_output() {
        self.active_inner = None;
      }
      return Ok(Some(value));
    }
    if !stream.has_pending_output() {
      self.active_inner = None;
    }
    Ok(None)
  }
}

impl<In, Out, Mat2, F> FlowLogic for SwitchMapLogic<In, Out, Mat2, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat2: Send + Sync + 'static,
  F: FnMut(In) -> Source<Out, Mat2> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    // 新しいサブストリームに切り替える（前のサブストリームはキャンセル）
    self.switch_to(value)?;
    if let Some(output) = self.pop_next_value()? {
      return Ok(vec![Box::new(output) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn can_accept_input(&self) -> bool {
    // 常に新しい外側要素を受け入れる（切り替えのため）
    true
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    if let Some(output) = self.pop_next_value()? {
      return Ok(vec![Box::new(output) as DynValue]);
    }
    Ok(Vec::new())
  }

  fn has_pending_output(&self) -> bool {
    self.active_inner.as_ref().is_some_and(SecondarySourceBridge::has_pending_output)
  }

  fn on_downstream_cancel(&mut self) -> Result<DownstreamCancelAction, StreamError> {
    // 下流キャンセル時はサブストリームも破棄
    self.active_inner = None;
    Ok(DownstreamCancelAction::Propagate)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.active_inner = None;
    Ok(())
  }
}
