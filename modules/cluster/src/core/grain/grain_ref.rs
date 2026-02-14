//! Grain reference entry point.

use alloc::{format, string::String};

use fraktor_actor_rs::core::{
  actor::{
    Pid,
    actor_ref::{ActorRefGeneric, ActorRefSender, SendOutcome},
  },
  error::SendError,
  event::stream::{EventStreamEvent, EventStreamSharedGeneric},
  futures::ActorFutureSharedGeneric,
  messaging::{AnyMessageGeneric, AskError, AskResponseGeneric, AskResult},
  scheduler::{ExecutionBatch, SchedulerCommand, SchedulerRunnable},
  system::{ActorSystemGeneric, state::SystemStateSharedGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeToolbox,
  sync::{ArcShared, SharedAccess},
};

use super::{
  GRAIN_EVENT_STREAM_NAME, GrainCallError, GrainCallOptions, GrainCodec, GrainEvent, GrainMetrics,
  GrainMetricsSharedGeneric, GrainResolvedRef,
};
use crate::core::{ClusterApiGeneric, ClusterRequestError, ClusterResolveError, identity::ClusterIdentity};

#[cfg(test)]
mod tests;

/// Grain reference entry point.
pub struct GrainRefGeneric<TB: RuntimeToolbox + 'static> {
  identity:     ClusterIdentity,
  api:          ClusterApiGeneric<TB>,
  options:      GrainCallOptions,
  codec:        Option<ArcShared<dyn GrainCodec<TB>>>,
  event_stream: EventStreamSharedGeneric<TB>,
  metrics:      Option<GrainMetricsSharedGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> GrainRefGeneric<TB> {
  /// Creates a new grain reference.
  #[must_use]
  pub fn new(api: ClusterApiGeneric<TB>, identity: ClusterIdentity) -> Self {
    let event_stream = api.system().event_stream();
    let metrics = api.grain_metrics_shared();
    Self { identity, api, options: GrainCallOptions::default(), codec: None, event_stream, metrics }
  }

  /// Applies call options to the grain reference.
  #[must_use]
  pub const fn with_options(mut self, options: GrainCallOptions) -> Self {
    self.options = options;
    self
  }

  /// Attaches a codec to validate serialization.
  #[must_use]
  pub fn with_codec(mut self, codec: ArcShared<dyn GrainCodec<TB>>) -> Self {
    self.codec = Some(codec);
    self
  }

  /// Returns the grain identity.
  #[must_use]
  pub const fn identity(&self) -> &ClusterIdentity {
    &self.identity
  }

  /// Resolves the grain reference.
  ///
  /// # Errors
  ///
  /// Returns an error if resolution fails.
  pub fn get(&self) -> Result<GrainResolvedRef<TB>, ClusterResolveError> {
    let actor_ref = self.resolve_with_retry()?;
    Ok(GrainResolvedRef { identity: self.identity.clone(), actor_ref })
  }

  /// Sends a request and returns the ask response.
  ///
  /// # Errors
  ///
  /// Returns an error if resolution or sending fails.
  pub fn request(&self, message: &AnyMessageGeneric<TB>) -> Result<AskResponseGeneric<TB>, GrainCallError> {
    self.request_internal(message, None)
  }

  /// Sends a request and returns the response future.
  ///
  /// The future resolves with `Ok(message)` on success, or `Err(AskError)` on failure.
  ///
  /// # Errors
  ///
  /// Returns an error if resolution or sending fails.
  pub fn request_future(
    &self,
    message: &AnyMessageGeneric<TB>,
  ) -> Result<ActorFutureSharedGeneric<AskResult<TB>, TB>, GrainCallError> {
    let response = self.request(message)?;
    let (_, future) = response.into_parts();
    Ok(future)
  }

  /// Sends a message with an explicit sender.
  ///
  /// # Errors
  ///
  /// Returns an error if resolution or sending fails.
  pub fn tell_with_sender(
    &self,
    message: &AnyMessageGeneric<TB>,
    sender: &ActorRefGeneric<TB>,
  ) -> Result<(), GrainCallError> {
    if let Err(error) = self.validate_codec(message) {
      self.publish_call_failed(&error);
      self.record_call_failed();
      return Err(error);
    }
    let actor_ref = match self.resolve_with_retry() {
      | Ok(actor_ref) => actor_ref,
      | Err(error) => {
        let wrapped = GrainCallError::ResolveFailed(error);
        self.publish_call_failed(&wrapped);
        self.record_call_failed();
        return Err(wrapped);
      },
    };
    let envelope = message.clone().with_sender(sender.clone());
    if let Err(error) = actor_ref.tell(envelope) {
      let call_error = GrainCallError::RequestFailed(ClusterRequestError::SendFailed { reason: format!("{error:?}") });
      self.publish_call_failed(&call_error);
      self.record_call_failed();
      return Err(call_error);
    }
    Ok(())
  }

  /// Sends a request with an explicit sender and returns the ask response.
  ///
  /// # Errors
  ///
  /// Returns an error if resolution or sending fails.
  pub fn request_with_sender(
    &self,
    message: &AnyMessageGeneric<TB>,
    sender: &ActorRefGeneric<TB>,
  ) -> Result<AskResponseGeneric<TB>, GrainCallError> {
    self.request_internal(message, Some(sender.clone()))
  }

  fn resolve_with_retry(&self) -> Result<ActorRefGeneric<TB>, ClusterResolveError> {
    let max_retries = self.options.retry.max_retries();
    let mut attempts = 0;
    loop {
      match self.api.get(&self.identity) {
        | Ok(actor_ref) => return Ok(actor_ref),
        | Err(ClusterResolveError::LookupPending) if attempts < max_retries => {
          attempts += 1;
          continue;
        },
        | Err(err) => return Err(err),
      }
    }
  }

  fn validate_codec(&self, message: &AnyMessageGeneric<TB>) -> Result<(), GrainCallError> {
    let Some(codec) = &self.codec else {
      return Ok(());
    };
    let encoded = codec.encode(message).map_err(GrainCallError::CodecFailed)?;
    let _ = codec.decode(&encoded).map_err(GrainCallError::CodecFailed)?;
    Ok(())
  }

  fn publish_call_failed(&self, error: &GrainCallError) {
    let reason = format!("{error:?}");
    let event = GrainEvent::CallFailed { identity: self.identity.clone(), reason };
    publish_grain_event(&self.event_stream, event);
  }

  fn record_call_failed(&self) {
    update_grain_metrics(&self.metrics, |metrics| metrics.record_call_failed());
  }

  fn request_internal(
    &self,
    message: &AnyMessageGeneric<TB>,
    forward_to: Option<ActorRefGeneric<TB>>,
  ) -> Result<AskResponseGeneric<TB>, GrainCallError> {
    if let Err(error) = self.validate_codec(message) {
      self.publish_call_failed(&error);
      self.record_call_failed();
      return Err(error);
    }
    let actor_ref = match self.resolve_with_retry() {
      | Ok(actor_ref) => actor_ref,
      | Err(error) => {
        let wrapped = GrainCallError::ResolveFailed(error);
        self.publish_call_failed(&wrapped);
        self.record_call_failed();
        return Err(wrapped);
      },
    };
    let state = self.api.system().state();
    let future = ActorFutureSharedGeneric::<AskResult<TB>, TB>::new();
    let reply_pid = state.allocate_pid();
    let reply_context = GrainReplyContext {
      identity:     self.identity.clone(),
      event_stream: self.event_stream.clone(),
      metrics:      self.metrics.clone(),
      state:        state.clone(),
      temp_pid:     Some(reply_pid),
    };
    let reply_sender = GrainReplySender::new(future.clone(), forward_to, reply_context);
    let reply_ref = ActorRefGeneric::with_system(reply_pid, reply_sender, &state);
    let temp_name = state.register_temp_actor(reply_ref.clone());
    let envelope = message.clone().with_sender(reply_ref.clone());
    if let Err(error) = actor_ref.tell(envelope) {
      state.unregister_temp_actor(&temp_name);
      let call_error = GrainCallError::RequestFailed(ClusterRequestError::SendFailed { reason: format!("{error:?}") });
      self.publish_call_failed(&call_error);
      self.record_call_failed();
      return Err(call_error);
    }
    state.register_ask_future(future.clone());
    let response = AskResponseGeneric::new(reply_ref, future);
    if let Some(timeout) = self.options.timeout {
      let reply_ref = response.sender().clone();
      let future = response.future().clone();
      let max_retries = self.options.retry.max_retries();
      let mut elapsed = timeout;
      let make_context = || GrainRetryContext {
        identity:     self.identity.clone(),
        event_stream: self.event_stream.clone(),
        metrics:      self.metrics.clone(),
        state:        state.clone(),
        temp_pid:     Some(reply_pid),
      };

      for attempt in 0..max_retries {
        let delay = self.options.retry.retry_delay(attempt);
        elapsed = elapsed.checked_add(delay).unwrap_or(elapsed);
        let runnable = ArcShared::new(GrainRetryRunnable::retry(
          make_context(),
          actor_ref.clone(),
          message.clone(),
          reply_ref.clone(),
          attempt,
          future.clone(),
        ));
        if let Err(error) = schedule_retry_with_system(self.api.system(), elapsed, runnable) {
          let call_error = GrainCallError::RequestFailed(error);
          self.publish_call_failed(&call_error);
          self.record_call_failed();
          return Err(call_error);
        }
        elapsed = elapsed.checked_add(timeout).unwrap_or(elapsed);
      }

      let runnable = ArcShared::new(GrainRetryRunnable::timeout(make_context(), future));
      if let Err(error) = schedule_retry_with_system(self.api.system(), elapsed, runnable) {
        state.unregister_temp_actor(&temp_name);
        let call_error = GrainCallError::RequestFailed(error);
        self.publish_call_failed(&call_error);
        self.record_call_failed();
        return Err(call_error);
      }
    }
    Ok(response)
  }
}

struct GrainRetryContext<TB: RuntimeToolbox + 'static> {
  identity:     ClusterIdentity,
  event_stream: EventStreamSharedGeneric<TB>,
  metrics:      Option<GrainMetricsSharedGeneric<TB>>,
  state:        SystemStateSharedGeneric<TB>,
  temp_pid:     Option<Pid>,
}

enum GrainRetryAction<TB: RuntimeToolbox + 'static> {
  Retry {
    actor_ref: ActorRefGeneric<TB>,
    message:   AnyMessageGeneric<TB>,
    reply_ref: ActorRefGeneric<TB>,
    attempt:   u32,
  },
  Timeout,
}

struct GrainRetryRunnable<TB: RuntimeToolbox + 'static> {
  future:  ActorFutureSharedGeneric<AskResult<TB>, TB>,
  context: GrainRetryContext<TB>,
  action:  GrainRetryAction<TB>,
}

impl<TB: RuntimeToolbox + 'static> GrainRetryRunnable<TB> {
  const fn retry(
    context: GrainRetryContext<TB>,
    actor_ref: ActorRefGeneric<TB>,
    message: AnyMessageGeneric<TB>,
    reply_ref: ActorRefGeneric<TB>,
    attempt: u32,
    future: ActorFutureSharedGeneric<AskResult<TB>, TB>,
  ) -> Self {
    Self { future, context, action: GrainRetryAction::Retry { actor_ref, message, reply_ref, attempt } }
  }

  const fn timeout(context: GrainRetryContext<TB>, future: ActorFutureSharedGeneric<AskResult<TB>, TB>) -> Self {
    Self { future, context, action: GrainRetryAction::Timeout }
  }
}

impl<TB: RuntimeToolbox + 'static> GrainRetryContext<TB> {
  fn cleanup_temp_reply(&self) {
    if let Some(pid) = &self.temp_pid {
      self.state.unregister_temp_actor_by_pid(pid);
    }
  }
}

impl<TB: RuntimeToolbox + 'static> SchedulerRunnable for GrainRetryRunnable<TB> {
  fn run(&self, _batch: &ExecutionBatch) {
    if self.future.with_read(|inner| inner.is_ready()) {
      self.context.cleanup_temp_reply();
      return;
    }

    match &self.action {
      | GrainRetryAction::Retry { actor_ref, message, reply_ref, attempt } => {
        let event = GrainEvent::CallRetrying { identity: self.context.identity.clone(), attempt: *attempt };
        publish_grain_event(&self.context.event_stream, event);
        update_grain_metrics(&self.context.metrics, |metrics| metrics.record_call_retried());
        let envelope = message.clone().with_sender(reply_ref.clone());
        if let Err(error) = actor_ref.tell(envelope) {
          let failure = ClusterRequestError::SendFailed { reason: format!("{error:?}") };
          let event =
            GrainEvent::CallFailed { identity: self.context.identity.clone(), reason: format!("{failure:?}") };
          publish_grain_event(&self.context.event_stream, event);
          update_grain_metrics(&self.context.metrics, |metrics| metrics.record_call_failed());
          complete_future(&self.future, &failure);
          self.context.cleanup_temp_reply();
        }
      },
      | GrainRetryAction::Timeout => {
        let event = GrainEvent::CallTimedOut { identity: self.context.identity.clone() };
        publish_grain_event(&self.context.event_stream, event);
        update_grain_metrics(&self.context.metrics, |metrics| metrics.record_call_timed_out());
        complete_future(&self.future, &ClusterRequestError::Timeout);
        self.context.cleanup_temp_reply();
      },
    }
  }
}

fn schedule_retry_with_system<TB: RuntimeToolbox + 'static>(
  system: &ActorSystemGeneric<TB>,
  wait: core::time::Duration,
  runnable: ArcShared<GrainRetryRunnable<TB>>,
) -> Result<(), ClusterRequestError> {
  let command = SchedulerCommand::RunRunnable { runnable, dispatcher: None };
  system
    .state()
    .scheduler()
    .with_write(|scheduler| scheduler.schedule_once(wait, command))
    .map(|_| ())
    .map_err(|error| ClusterRequestError::TimeoutScheduleFailed { reason: format!("{error:?}") })
}

fn complete_future<TB: RuntimeToolbox + 'static>(
  future: &ActorFutureSharedGeneric<AskResult<TB>, TB>,
  error: &ClusterRequestError,
) {
  // ClusterRequestError を AskError に変換
  let ask_error = match error {
    | ClusterRequestError::Timeout => AskError::Timeout,
    | ClusterRequestError::ResolveFailed(_) => AskError::DeadLetter,
    | ClusterRequestError::SendFailed { .. } => AskError::SendFailed,
    | ClusterRequestError::TimeoutScheduleFailed { .. } => AskError::SendFailed,
  };

  let waker = future.with_write(|inner| if inner.is_ready() { None } else { inner.complete(Err(ask_error)) });
  if let Some(waker) = waker {
    waker.wake();
  }
}

fn publish_grain_event<TB: RuntimeToolbox + 'static>(event_stream: &EventStreamSharedGeneric<TB>, event: GrainEvent) {
  let payload = AnyMessageGeneric::new(event);
  let extension_event = EventStreamEvent::Extension { name: String::from(GRAIN_EVENT_STREAM_NAME), payload };
  event_stream.publish(&extension_event);
}

fn update_grain_metrics<TB: RuntimeToolbox + 'static>(
  metrics: &Option<GrainMetricsSharedGeneric<TB>>,
  f: impl FnOnce(&mut GrainMetrics),
) {
  if let Some(metrics) = metrics {
    metrics.with_write(|inner| f(inner));
  }
}

struct GrainReplySender<TB: RuntimeToolbox + 'static> {
  future:     ActorFutureSharedGeneric<AskResult<TB>, TB>,
  forward_to: Option<ActorRefGeneric<TB>>,
  context:    GrainReplyContext<TB>,
}

impl<TB: RuntimeToolbox + 'static> GrainReplySender<TB> {
  const fn new(
    future: ActorFutureSharedGeneric<AskResult<TB>, TB>,
    forward_to: Option<ActorRefGeneric<TB>>,
    context: GrainReplyContext<TB>,
  ) -> Self {
    Self { future, forward_to, context }
  }
}

impl<TB: RuntimeToolbox + 'static> ActorRefSender<TB> for GrainReplySender<TB> {
  fn send(&mut self, message: AnyMessageGeneric<TB>) -> Result<SendOutcome, SendError<TB>> {
    if self.future.with_read(|inner| inner.is_ready()) {
      self.context.cleanup_temp_reply();
      return Ok(SendOutcome::Delivered);
    }

    if let Some(target) = &self.forward_to
      && let Err(error) = target.tell(message.clone())
    {
      let failure = ClusterRequestError::SendFailed { reason: format!("{error:?}") };
      self.context.publish_call_failed(&failure);
      complete_future(&self.future, &failure);
      self.context.cleanup_temp_reply();
      return Ok(SendOutcome::Delivered);
    }

    let waker = self.future.with_write(|inner| inner.complete(Ok(message)));
    if let Some(waker) = waker {
      waker.wake();
    }
    self.context.cleanup_temp_reply();
    Ok(SendOutcome::Delivered)
  }
}

struct GrainReplyContext<TB: RuntimeToolbox + 'static> {
  identity:     ClusterIdentity,
  event_stream: EventStreamSharedGeneric<TB>,
  metrics:      Option<GrainMetricsSharedGeneric<TB>>,
  state:        SystemStateSharedGeneric<TB>,
  temp_pid:     Option<Pid>,
}

impl<TB: RuntimeToolbox + 'static> GrainReplyContext<TB> {
  fn cleanup_temp_reply(&self) {
    if let Some(pid) = &self.temp_pid {
      self.state.unregister_temp_actor_by_pid(pid);
    }
  }

  fn publish_call_failed(&self, error: &ClusterRequestError) {
    let event = GrainEvent::CallFailed { identity: self.identity.clone(), reason: format!("{error:?}") };
    publish_grain_event(&self.event_stream, event);
    update_grain_metrics(&self.metrics, |metrics| metrics.record_call_failed());
  }
}
