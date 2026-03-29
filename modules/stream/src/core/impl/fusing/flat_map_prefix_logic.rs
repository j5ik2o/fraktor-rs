use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use crate::core::{DynValue, FlowLogic, StageDefinition, StreamError, downcast_value, stage::Flow};

pub(in crate::core) struct FlatMapPrefixLogic<In, Out, Mat, F> {
  pub(in crate::core) prefix_len:    usize,
  pub(in crate::core) factory:       F,
  pub(in crate::core) prefix_values: Vec<In>,
  pub(in crate::core) inner_logics:  Vec<Box<dyn FlowLogic>>,
  pub(in crate::core) factory_built: bool,
  pub(in crate::core) source_done:   bool,
  pub(in crate::core) _pd:           PhantomData<fn(In) -> (Out, Mat)>,
}

impl<In, Out, Mat, F> FlatMapPrefixLogic<In, Out, Mat, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat: Send + Sync + 'static,
  F: FnMut(Vec<In>) -> Flow<In, Out, Mat> + Send + Sync + 'static,
{
  fn build_inner_if_needed(&mut self) {
    if self.factory_built {
      return;
    }
    if !self.source_done && self.prefix_values.len() < self.prefix_len {
      return;
    }

    let prefix = core::mem::take(&mut self.prefix_values);
    let flow = (self.factory)(prefix);
    let (graph, _mat) = flow.into_parts();
    for stage in graph.into_stages() {
      if let StageDefinition::Flow(definition) = stage {
        self.inner_logics.push(definition.logic);
      }
    }
    self.factory_built = true;
  }

  fn apply_inner(&mut self, value: In) -> Result<Vec<DynValue>, StreamError> {
    let mut values = vec![Box::new(value) as DynValue];
    for logic in &mut self.inner_logics {
      let mut next = Vec::new();
      for current in values {
        next.extend(logic.apply(current)?);
      }
      values = next;
    }
    Ok(values)
  }
}

impl<In, Out, Mat, F> FlowLogic for FlatMapPrefixLogic<In, Out, Mat, F>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  Mat: Send + Sync + 'static,
  F: FnMut(Vec<In>) -> Flow<In, Out, Mat> + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;

    if !self.factory_built && self.prefix_values.len() < self.prefix_len {
      self.prefix_values.push(value);
      self.build_inner_if_needed();
      return Ok(Vec::new());
    }

    self.build_inner_if_needed();
    self.apply_inner(value)
  }

  fn can_accept_input(&self) -> bool {
    self.inner_logics.first().is_none_or(|logic| logic.can_accept_input())
  }

  fn on_source_done(&mut self) -> Result<(), StreamError> {
    self.source_done = true;
    self.build_inner_if_needed();
    for logic in &mut self.inner_logics {
      logic.on_source_done()?;
    }
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let mut result = Vec::new();
    for start in 0..self.inner_logics.len() {
      let mut values = self.inner_logics[start].drain_pending()?;
      for index in (start + 1)..self.inner_logics.len() {
        let mut next = Vec::new();
        for value in values {
          next.extend(self.inner_logics[index].apply(value)?);
        }
        values = next;
      }
      result.extend(values);
    }
    Ok(result)
  }

  fn has_pending_output(&self) -> bool {
    self.inner_logics.iter().any(|logic| logic.has_pending_output())
  }

  fn take_shutdown_request(&mut self) -> bool {
    let mut requested = false;
    for logic in &mut self.inner_logics {
      requested |= logic.take_shutdown_request();
    }
    requested
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.prefix_values.clear();
    self.inner_logics.clear();
    self.factory_built = false;
    self.source_done = false;
    Ok(())
  }
}
