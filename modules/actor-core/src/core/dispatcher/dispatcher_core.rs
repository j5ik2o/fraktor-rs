#[cfg(test)]
mod tests;

use alloc::format;
use core::{
  num::NonZeroUsize,
  pin::Pin,
  sync::atomic::Ordering,
  task::{Context, Poll},
  time::Duration,
};

use fraktor_utils_core_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};
use portable_atomic::{AtomicU8, AtomicU64};

use super::{
  dispatch_error::DispatchError, dispatch_executor::DispatchExecutor, dispatcher_dump_event::DispatcherDumpEvent,
  dispatcher_state::DispatcherState, schedule_adapter::ScheduleAdapter,
};
use crate::core::{
  error::{ActorError, SendError},
  event_stream::EventStreamEvent,
  logging::LogLevel,
  mailbox::{
    EnqueueOutcome, MailboxGeneric, MailboxMessage, MailboxOfferFutureGeneric, MailboxPressureEvent, ScheduleHints,
  },
  messaging::{AnyMessageGeneric, SystemMessage, message_invoker::MessageInvoker},
  system::SystemStateGeneric,
};

const DEFAULT_THROUGHPUT: usize = 300;
pub(crate) const MAX_EXECUTOR_RETRIES: usize = 2;

/// Entity that drains the mailbox and invokes messages.
pub(crate) struct DispatcherCore<TB: RuntimeToolbox + 'static> {
  mailbox:             ArcShared<MailboxGeneric<TB>>,
  executor:            ArcShared<dyn DispatchExecutor<TB>>,
  schedule_adapter:    ArcShared<dyn ScheduleAdapter<TB>>,
  invoker:             ToolboxMutex<Option<ArcShared<dyn MessageInvoker<TB>>>, TB>,
  state:               AtomicU8,
  throughput_limit:    Option<NonZeroUsize>,
  throughput_deadline: Option<Duration>,
  starvation_deadline: Option<Duration>,
  system_state:        Option<ArcShared<SystemStateGeneric<TB>>>,
  last_progress:       AtomicU64,
}

unsafe impl<TB: RuntimeToolbox + 'static> Send for DispatcherCore<TB> {}
unsafe impl<TB: RuntimeToolbox + 'static> Sync for DispatcherCore<TB> {}

impl<TB: RuntimeToolbox + 'static> DispatcherCore<TB> {
  pub(crate) fn new(
    mailbox: ArcShared<MailboxGeneric<TB>>,
    executor: ArcShared<dyn DispatchExecutor<TB>>,
    schedule_adapter: ArcShared<dyn ScheduleAdapter<TB>>,
    throughput_limit: Option<NonZeroUsize>,
    throughput_deadline: Option<Duration>,
    starvation_deadline: Option<Duration>,
  ) -> Self {
    let system_state = mailbox.system_state();
    Self {
      mailbox,
      executor,
      schedule_adapter,
      invoker: <TB::MutexFamily as SyncMutexFamily>::create(None),
      state: AtomicU8::new(DispatcherState::Idle.as_u8()),
      throughput_limit,
      throughput_deadline,
      starvation_deadline,
      system_state,
      last_progress: AtomicU64::new(0),
    }
  }

  pub(crate) const fn mailbox(&self) -> &ArcShared<MailboxGeneric<TB>> {
    &self.mailbox
  }

  pub(crate) fn register_invoker(&self, invoker: ArcShared<dyn MessageInvoker<TB>>) {
    *self.invoker.lock() = Some(invoker);
  }

  pub(crate) fn executor(&self) -> &ArcShared<dyn DispatchExecutor<TB>> {
    &self.executor
  }

  pub(crate) fn schedule_adapter(&self) -> ArcShared<dyn ScheduleAdapter<TB>> {
    self.schedule_adapter.clone()
  }

  pub(crate) const fn state(&self) -> &AtomicU8 {
    &self.state
  }

  fn record_progress(&self) {
    if let Some(state) = &self.system_state {
      let tick = duration_to_millis(state.monotonic_now());
      self.last_progress.store(tick, Ordering::Release);
    }
  }

  fn elapsed_since_progress(&self) -> Option<Duration> {
    let last = self.last_progress.load(Ordering::Acquire);
    if last == 0 {
      return None;
    }
    let state = self.system_state.as_ref()?;
    let now = duration_to_millis(state.monotonic_now());
    let delta = now.saturating_sub(last);
    Some(Duration::from_millis(delta))
  }

  pub(crate) fn drive(self_arc: &ArcShared<Self>) {
    self_arc.mailbox.set_running();
    loop {
      {
        let this = self_arc;
        this.process_batch();
      }

      let should_continue = {
        let this = self_arc;
        DispatcherState::Idle.store(&this.state);
        this.has_pending_work()
          && DispatcherState::compare_exchange(DispatcherState::Idle, DispatcherState::Running, &this.state).is_ok()
      };

      if should_continue {
        self_arc.mailbox.set_running();
        continue;
      }

      let pending_reschedule = self_arc.mailbox.set_idle();
      if pending_reschedule {
        let hints = self_arc.mailbox.current_schedule_hints();
        Self::request_execution(self_arc, hints);
      }

      break;
    }
  }

  fn process_batch(&self) {
    let limit = self.throughput_limit.map(NonZeroUsize::get).unwrap_or(DEFAULT_THROUGHPUT);
    let mut processed = 0_usize;
    let deadline_anchor = self.deadline_anchor();

    while processed < limit {
      if self.deadline_reached(deadline_anchor, processed) {
        break;
      }
      match self.mailbox.dequeue() {
        | Some(MailboxMessage::System(msg)) => {
          self.handle_system_message(msg);
          self.record_progress();
          processed += 1;
        },
        | Some(MailboxMessage::User(msg)) => {
          self.handle_user_message(msg);
          self.record_progress();
          processed += 1;
        },
        | None => break,
      }
    }
  }

  fn deadline_anchor(&self) -> Option<Duration> {
    if self.throughput_deadline.is_some() {
      self.system_state.as_ref().map(|state| state.monotonic_now())
    } else {
      None
    }
  }

  fn deadline_reached(&self, anchor: Option<Duration>, processed: usize) -> bool {
    if processed == 0 {
      return false;
    }
    match (self.throughput_deadline, anchor, self.system_state.as_ref()) {
      | (Some(limit), Some(start), Some(state)) => state.monotonic_now().saturating_sub(start) >= limit,
      | _ => false,
    }
  }

  fn handle_system_message(&self, message: SystemMessage) {
    match message {
      | SystemMessage::Suspend => self.mailbox.suspend(),
      | SystemMessage::Resume => self.mailbox.resume(),
      | other => {
        let _ = self.invoke_system_message(other);
      },
    }
  }

  fn handle_user_message(&self, message: AnyMessageGeneric<TB>) {
    let _ = self.invoke_user_message(message);
  }

  fn invoke_user_message(&self, message: AnyMessageGeneric<TB>) -> Result<(), ActorError> {
    if let Some(invoker) = self.invoker.lock().as_ref() {
      return invoker.invoke_user_message(message);
    }
    Ok(())
  }

  fn invoke_system_message(&self, message: SystemMessage) -> Result<(), ActorError> {
    if let Some(invoker) = self.invoker.lock().as_ref() {
      return invoker.invoke_system_message(message);
    }
    Ok(())
  }

  #[allow(dead_code)]
  pub(crate) fn enqueue_user(self_arc: &ArcShared<Self>, message: AnyMessageGeneric<TB>) -> Result<(), SendError<TB>> {
    match self_arc.mailbox.enqueue_user(message) {
      | Ok(EnqueueOutcome::Enqueued) => {
        Self::request_execution(self_arc, ScheduleHints {
          has_system_messages: false,
          has_user_messages:   true,
          backpressure_active: false,
        });
        Ok(())
      },
      | Ok(EnqueueOutcome::Pending(mut future)) => {
        Self::drain_offer_future(self_arc, &mut future)?;
        Self::request_execution(self_arc, ScheduleHints {
          has_system_messages: false,
          has_user_messages:   true,
          backpressure_active: false,
        });
        Ok(())
      },
      | Err(error) => Err(error),
    }
  }

  pub(crate) fn enqueue_system(self_arc: &ArcShared<Self>, message: SystemMessage) -> Result<(), SendError<TB>> {
    self_arc.mailbox.enqueue_system(message)?;
    Self::request_execution(self_arc, ScheduleHints {
      has_system_messages: true,
      has_user_messages:   false,
      backpressure_active: false,
    });
    Ok(())
  }

  #[allow(dead_code)]
  fn drain_offer_future(
    self_arc: &ArcShared<Self>,
    future: &mut MailboxOfferFutureGeneric<TB>,
  ) -> Result<(), SendError<TB>> {
    let adapter = self_arc.schedule_adapter();
    let dispatcher = super::base::DispatcherGeneric::from_core(self_arc.clone());
    let waker = adapter.create_waker(dispatcher);
    let mut cx = Context::from_waker(&waker);

    loop {
      match Pin::new(&mut *future).poll(&mut cx) {
        | Poll::Ready(Ok(_)) => return Ok(()),
        | Poll::Ready(Err(error)) => return Err(error),
        | Poll::Pending => {
          Self::request_execution(self_arc, ScheduleHints {
            has_system_messages: false,
            has_user_messages:   true,
            backpressure_active: false,
          });
          adapter.on_pending();
        },
      }
    }
  }

  fn has_pending_work(&self) -> bool {
    self.mailbox.system_len() > 0 || (!self.mailbox.is_suspended() && self.mailbox.user_len() > 0)
  }

  pub(crate) fn request_execution(self_arc: &ArcShared<Self>, hints: ScheduleHints) {
    if !hints.has_system_messages && !hints.has_user_messages && !hints.backpressure_active {
      return;
    }
    if self_arc.mailbox.request_schedule(hints) {
      super::base::DispatcherGeneric::from_core(self_arc.clone()).spawn_execution();
    } else {
      self_arc.handle_starvation(hints);
    }
  }

  pub(crate) fn handle_backpressure(self_arc: &ArcShared<Self>, _event: &MailboxPressureEvent) {
    let hints = ScheduleHints { has_system_messages: false, has_user_messages: true, backpressure_active: true };
    Self::request_execution(self_arc, hints);
  }

  pub(crate) fn handle_executor_failure(&self, attempts: usize, error: DispatchError) {
    DispatcherState::Idle.store(self.state());
    let _ = self.mailbox.set_idle();
    self.schedule_adapter.notify_rejected(attempts);
    let message = format!("dispatcher execution failed after {} attempt(s): {}", attempts, error);
    self.mailbox.emit_log(LogLevel::Error, message);
  }

  pub(crate) fn publish_dump(self_arc: &ArcShared<Self>) {
    let Some(system_state) = self_arc.mailbox.system_state() else {
      return;
    };
    let Some(pid) = self_arc.mailbox.pid() else {
      return;
    };
    let state_value = self_arc.state.load(Ordering::Acquire);
    let dispatcher_state = DispatcherState::from_u8(state_value);
    let event = DispatcherDumpEvent::new(
      pid,
      self_arc.mailbox.user_len(),
      self_arc.mailbox.system_len(),
      matches!(dispatcher_state, DispatcherState::Running),
      self_arc.mailbox.is_suspended(),
    );
    system_state.publish_event(&EventStreamEvent::DispatcherDump(event));
  }
}

const fn duration_to_millis(duration: Duration) -> u64 {
  duration.as_millis() as u64
}

impl<TB: RuntimeToolbox + 'static> DispatcherCore<TB> {
  fn handle_starvation(&self, hints: ScheduleHints) {
    if !hints.has_system_messages && !hints.has_user_messages && !hints.backpressure_active {
      return;
    }
    let Some(threshold) = self.starvation_deadline else {
      return;
    };
    if let Some(elapsed) = self.elapsed_since_progress().filter(|elapsed| *elapsed >= threshold) {
      let message = format!("dispatcher detected potential starvation after {:?}", elapsed);
      self.mailbox.emit_log(LogLevel::Warn, message);
    }
  }
}
