use alloc::boxed::Box;
use core::marker::PhantomData;

use crate::{
  DemandTracker, DynValue, SinkDecision, SinkLogic, StageDefinition, StreamError, dsl::Sink,
  materialization::StreamFuture,
};

/// Sink logic that materializes its inner sink from a factory.
pub(crate) struct MaterializedSinkLogic<In, Out, F> {
  factory:          Option<F>,
  inner:            Option<Box<dyn SinkLogic>>,
  inner_completion: Option<StreamFuture<Out>>,
  completion:       StreamFuture<Out>,
  completed:        bool,
  _pd:              PhantomData<fn(In)>,
}

impl<In, Out, F> MaterializedSinkLogic<In, Out, F> {
  /// Creates materialized sink logic.
  pub(crate) const fn new(factory: F, completion: StreamFuture<Out>) -> Self {
    Self { factory: Some(factory), inner: None, inner_completion: None, completion, completed: false, _pd: PhantomData }
  }

  fn mirror_inner_completion(&mut self) {
    if self.completed {
      return;
    }
    if let Some(result) = self.inner_completion.as_ref().and_then(StreamFuture::try_take) {
      self.completion.complete(result);
      self.completed = true;
    }
  }

  fn complete_with_error(&mut self, error: StreamError) -> StreamError {
    if !self.completed {
      self.completion.complete(Err(error.clone()));
      self.completed = true;
    }
    error
  }
}

impl<In, Out, F> MaterializedSinkLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + 'static,
  F: FnOnce() -> Sink<In, StreamFuture<Out>> + Send + 'static,
{
  fn materialize_inner(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    if self.inner.is_some() {
      return Ok(());
    }
    let Some(factory) = self.factory.take() else {
      return Err(self.complete_with_error(StreamError::Failed));
    };
    let sink = factory();
    let (graph, inner_completion) = sink.into_parts();
    self.inner_completion = Some(inner_completion);
    for stage in graph.into_stages() {
      if let StageDefinition::Sink(definition) = stage {
        self.inner = Some(definition.logic);
        break;
      }
    }
    let Some(inner) = self.inner.as_mut() else {
      return Err(self.complete_with_error(StreamError::InvalidConnection));
    };
    inner.on_start(demand)?;
    self.mirror_inner_completion();
    Ok(())
  }
}

impl<In, Out, F> SinkLogic for MaterializedSinkLogic<In, Out, F>
where
  In: Send + Sync + 'static,
  Out: Send + 'static,
  F: FnOnce() -> Sink<In, StreamFuture<Out>> + Send + 'static,
{
  fn can_accept_input(&self) -> bool {
    self.inner.as_ref().is_none_or(|inner| inner.can_accept_input())
  }

  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    self.materialize_inner(demand)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    self.materialize_inner(demand)?;
    let Some(inner) = self.inner.as_mut() else {
      return Err(self.complete_with_error(StreamError::InvalidConnection));
    };
    let decision = inner.on_push(input, demand)?;
    self.mirror_inner_completion();
    Ok(decision)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    if self.completed {
      return Ok(());
    }
    let result = match self.inner.as_mut() {
      | Some(inner) => inner.on_complete(),
      | None => Ok(()),
    };
    if let Err(error) = &result {
      self.complete_with_error(error.clone());
    }
    self.mirror_inner_completion();
    result
  }

  fn on_error(&mut self, error: StreamError) {
    if let Some(inner) = self.inner.as_mut() {
      inner.on_error(error.clone());
    }
    if !self.completed {
      self.completion.complete(Err(error));
      self.completed = true;
    }
  }

  fn on_tick(&mut self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    let ticked = match self.inner.as_mut() {
      | Some(inner) => inner.on_tick(demand)?,
      | None => false,
    };
    self.mirror_inner_completion();
    Ok(ticked)
  }

  fn on_upstream_finish(&mut self) -> Result<bool, StreamError> {
    let finished = match self.inner.as_mut() {
      | Some(inner) => inner.on_upstream_finish()?,
      | None => false,
    };
    self.mirror_inner_completion();
    Ok(finished)
  }

  fn has_pending_work(&self) -> bool {
    self.inner.as_ref().is_some_and(|inner| inner.has_pending_work())
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    if let Some(inner) = self.inner.as_mut() {
      inner.on_restart()?;
    }
    self.completed = false;
    Ok(())
  }
}
