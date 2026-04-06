use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use crate::core::{DynValue, FlowLogic, StageDefinition, StreamError, dsl::Flow};

pub(in crate::core) struct LazyFlowLogic<In, Out, Mat, F> {
  pub(in crate::core) factory:      Option<F>,
  pub(in crate::core) inner_logics: Vec<Box<dyn FlowLogic>>,
  // factory 生成 Flow の Mat 値を保持
  pub(in crate::core) mat:          Option<Mat>,
  pub(in crate::core) _pd:          PhantomData<fn(In, Out)>,
}

impl<In, Out, Mat, F> FlowLogic for LazyFlowLogic<In, Out, Mat, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat: Default + Send + 'static,
  F: FnOnce() -> Flow<In, Out, Mat> + Send + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if let Some(factory) = self.factory.take() {
      let flow = factory();
      let (graph, mat) = flow.into_parts();
      self.mat = Some(mat);
      let stages = graph.into_stages();
      for stage in stages {
        if let StageDefinition::Flow(def) = stage {
          self.inner_logics.push(def.logic);
        }
      }
    }

    if self.inner_logics.is_empty() {
      return Ok(vec![input]);
    }

    let mut values = vec![input];
    for logic in &mut self.inner_logics {
      let mut next = Vec::new();
      for v in values {
        next.extend(logic.apply(v)?);
      }
      values = next;
    }
    Ok(values)
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    for logic in &mut self.inner_logics {
      logic.on_tick(tick_count)?;
    }
    Ok(())
  }

  fn can_accept_input(&self) -> bool {
    self.inner_logics.first().is_none_or(|l| l.can_accept_input())
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    for logic in &mut self.inner_logics {
      logic.on_source_done()?;
    }
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let n = self.inner_logics.len();
    let mut result = Vec::new();
    for start in 0..n {
      let mut values = self.inner_logics[start].drain_pending()?;
      for j in (start + 1)..n {
        let mut next = Vec::new();
        for v in values {
          next.extend(self.inner_logics[j].apply(v)?);
        }
        values = next;
      }
      result.extend(values);
    }
    Ok(result)
  }

  fn has_pending_output(&self) -> bool {
    self.inner_logics.iter().any(|l| l.has_pending_output())
  }

  fn take_shutdown_request(&mut self) -> bool {
    // any() の短絡評価を避け、全 inner logic のフラグを一括クリアする
    self.inner_logics.iter_mut().fold(false, |acc, l| l.take_shutdown_request() || acc)
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    for logic in &mut self.inner_logics {
      logic.on_restart()?;
    }
    Ok(())
  }
}
