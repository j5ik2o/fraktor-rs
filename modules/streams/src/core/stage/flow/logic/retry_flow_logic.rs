use alloc::{boxed::Box, collections::VecDeque, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

/// Flow logic that retries individual elements with exponential backoff.
///
/// When `decide_retry` returns `Some(retry_element)` for an output, the element
/// is re-applied through the inner flow logics after a backoff delay. The
/// backoff doubles on each retry (clamped to `max_backoff_ticks`) with optional
/// jitter.
///
/// Each pending retry carries its own retry count and backoff state so that
/// sibling retries do not starve each other's retry budget.
///
/// # Experimental
///
/// This corresponds to Pekko's `@ApiMayChange` `RetryFlow` and may change in
/// future releases.
pub(in crate::core::stage::flow) struct RetryFlowLogic<In, Out, R> {
  inner_logics:           Vec<Box<dyn FlowLogic>>,
  decide_retry:           R,
  element_in_progress:    Option<In>,
  active_retry_count:     usize,
  active_backoff_ticks:   u32,
  max_retries:            usize,
  min_backoff_ticks:      u32,
  max_backoff_ticks:      u32,
  random_factor_permille: u16,
  jitter_state:           u64,
  pending_retries:        VecDeque<PendingRetry<In>>,
  tick_count:             u64,
  _pd:                    PhantomData<fn() -> Out>,
}

struct PendingRetry<In> {
  element:       In,
  ready_at:      u64,
  retry_count:   usize,
  backoff_ticks: u32,
}

impl<In, Out, R> RetryFlowLogic<In, Out, R>
where
  In: Clone + Send + Sync + 'static,
  Out: Send + Sync + 'static,
  R: Fn(&In, &Out) -> Option<In> + Send + 'static,
{
  pub(in crate::core::stage::flow) fn new(
    inner_logics: Vec<Box<dyn FlowLogic>>,
    decide_retry: R,
    max_retries: usize,
    min_backoff_ticks: u32,
    max_backoff_ticks: u32,
    random_factor_permille: u16,
  ) -> Self {
    Self {
      inner_logics,
      decide_retry,
      element_in_progress: None,
      active_retry_count: 0,
      active_backoff_ticks: min_backoff_ticks,
      max_retries,
      min_backoff_ticks,
      max_backoff_ticks,
      random_factor_permille,
      jitter_state: 0xcafe_u64,
      pending_retries: VecDeque::new(),
      tick_count: 0,
      _pd: PhantomData,
    }
  }

  fn apply_through_inner(&mut self, input: In) -> Result<Vec<DynValue>, StreamError> {
    let mut current: Vec<DynValue> = vec![Box::new(input) as DynValue];
    for logic in &mut self.inner_logics {
      let mut next = Vec::new();
      for value in current {
        next.extend(logic.apply(value)?);
      }
      current = next;
    }
    Ok(current)
  }

  fn process_element(&mut self, input: In, retry_count: usize, backoff_ticks: u32) -> Result<Vec<DynValue>, StreamError> {
    self.element_in_progress = Some(input.clone());
    self.active_retry_count = retry_count;
    self.active_backoff_ticks = backoff_ticks;
    let outputs = self.apply_through_inner(input)?;
    self.check_outputs(outputs)
  }

  fn check_outputs(&mut self, outputs: Vec<DynValue>) -> Result<Vec<DynValue>, StreamError> {
    if outputs.is_empty() {
      self.element_in_progress = None;
      return Ok(outputs);
    }
    let Some(element) = self.element_in_progress.clone() else {
      return Err(StreamError::Failed);
    };
    let mut result = Vec::new();
    let mut scheduled_retry = false;
    for output in outputs {
      let out_value = output.downcast::<Out>().map_err(|_| StreamError::TypeMismatch)?;
      if let Some(retry_elem) = (self.decide_retry)(&element, &out_value) {
        if self.active_retry_count >= self.max_retries {
          result.push(Box::new(*out_value) as DynValue);
        } else {
          let retry_count = self.active_retry_count.saturating_add(1);
          let cooldown = self.next_cooldown_ticks();
          let backoff_ticks = self.active_backoff_ticks;
          let ready_at = self.tick_count.saturating_add(u64::from(cooldown));
          self.pending_retries.push_back(PendingRetry { element: retry_elem, ready_at, retry_count, backoff_ticks });
          scheduled_retry = true;
        }
      } else {
        result.push(Box::new(*out_value) as DynValue);
      }
    }
    self.element_in_progress = None;
    if scheduled_retry {
      self.restart_inner_logics()?;
    }
    Ok(result)
  }

  fn next_cooldown_ticks(&mut self) -> u32 {
    let base = self.active_backoff_ticks.max(self.min_backoff_ticks).min(self.max_backoff_ticks);
    let jitter_ticks = self.compute_jitter_ticks(base);
    self.active_backoff_ticks = base.saturating_mul(2).min(self.max_backoff_ticks).max(self.min_backoff_ticks);
    base.saturating_add(jitter_ticks).min(self.max_backoff_ticks)
  }

  fn compute_jitter_ticks(&mut self, base_ticks: u32) -> u32 {
    let factor = u32::from(self.random_factor_permille);
    if factor == 0 || base_ticks == 0 {
      return 0;
    }
    self.jitter_state = self.jitter_state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let ratio_permille = (self.jitter_state >> 32) as u32 % 1001;
    base_ticks.saturating_mul(factor).saturating_mul(ratio_permille) / 1_000_000
  }

  fn restart_inner_logics(&mut self) -> Result<(), StreamError> {
    for logic in &mut self.inner_logics {
      logic.on_restart()?;
    }
    Ok(())
  }
}

impl<In, Out, R> FlowLogic for RetryFlowLogic<In, Out, R>
where
  In: Clone + Send + Sync + 'static,
  Out: Send + Sync + 'static,
  R: Fn(&In, &Out) -> Option<In> + Send + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    self.process_element(value, 0, self.min_backoff_ticks)
  }

  fn on_tick(&mut self, tick_count: u64) -> Result<(), StreamError> {
    self.tick_count = tick_count;
    Ok(())
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let Some(pending) = self.pending_retries.front() else {
      return Ok(Vec::new());
    };
    if pending.ready_at > self.tick_count {
      return Ok(Vec::new());
    }
    let Some(pending) = self.pending_retries.pop_front() else {
      return Ok(Vec::new());
    };
    self.process_element(pending.element, pending.retry_count, pending.backoff_ticks)
  }

  fn has_pending_output(&self) -> bool {
    !self.pending_retries.is_empty()
  }

  fn can_accept_input(&self) -> bool {
    self.pending_retries.is_empty()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.element_in_progress = None;
    self.active_retry_count = 0;
    self.active_backoff_ticks = self.min_backoff_ticks;
    self.pending_retries.clear();
    self.tick_count = 0;
    self.restart_inner_logics()
  }
}
