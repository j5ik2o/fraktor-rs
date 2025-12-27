//! At-least-once delivery helper for persistent actors.

use alloc::{format, vec::Vec};
use core::{any::Any, marker::PhantomData, time::Duration};

use fraktor_actor_rs::core::{
  actor::{ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
  scheduler::{SchedulerCommand, SchedulerError, SchedulerHandle},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox},
  sync::{ArcShared, SharedAccess},
  time::{MonotonicClock, TimerInstant},
};

use crate::core::{
  at_least_once_delivery_config::AtLeastOnceDeliveryConfig,
  at_least_once_delivery_snapshot::AtLeastOnceDeliverySnapshot, unconfirmed_delivery::UnconfirmedDelivery,
};

/// At-least-once delivery helper type alias for the default toolbox.
pub type AtLeastOnceDelivery = AtLeastOnceDeliveryGeneric<NoStdToolbox>;

/// Maintains delivery state and schedules redelivery.
pub struct AtLeastOnceDeliveryGeneric<TB: RuntimeToolbox + 'static> {
  config:            AtLeastOnceDeliveryConfig,
  next_delivery_id:  u64,
  unconfirmed:       Vec<UnconfirmedDelivery<TB>>,
  redelivery_handle: Option<SchedulerHandle>,
  _marker:           PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> AtLeastOnceDeliveryGeneric<TB> {
  /// Creates a new helper with the provided configuration.
  #[must_use]
  pub const fn new(config: AtLeastOnceDeliveryConfig) -> Self {
    Self { config, next_delivery_id: 1, unconfirmed: Vec::new(), redelivery_handle: None, _marker: PhantomData }
  }

  /// Returns the current delivery id.
  #[must_use]
  pub const fn current_delivery_id(&self) -> u64 {
    self.next_delivery_id.saturating_sub(1)
  }

  /// Returns the number of unconfirmed deliveries.
  #[must_use]
  pub const fn number_of_unconfirmed(&self) -> usize {
    self.unconfirmed.len()
  }

  /// Sends a message with at-least-once semantics.
  ///
  /// # Errors
  ///
  /// Returns an error when the maximum number of unconfirmed messages is exceeded
  /// or when sending fails.
  pub fn deliver<M, F>(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    destination: ActorRefGeneric<TB>,
    make_message: F,
  ) -> Result<u64, ActorError>
  where
    M: Any + Send + Sync + 'static,
    F: FnOnce(u64) -> M, {
    if self.unconfirmed.len() >= self.config.max_unconfirmed() {
      return Err(ActorError::recoverable("max unconfirmed messages exceeded"));
    }
    let delivery_id = self.next_delivery_id;
    self.next_delivery_id = self.next_delivery_id.saturating_add(1);
    let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(make_message(delivery_id));
    let sender = ctx.self_ref();
    let message = AnyMessageGeneric::from_erased(payload.clone(), Some(sender.clone()));
    destination.tell(message).map_err(|error| ActorError::from_send_error(&error))?;
    let timestamp = now_instant(ctx);
    self.unconfirmed.push(UnconfirmedDelivery::new(delivery_id, destination, payload, Some(sender), timestamp));
    self.ensure_redelivery_scheduled(ctx)?;
    Ok(delivery_id)
  }

  /// Confirms a delivery and cancels it from redelivery tracking.
  pub fn confirm_delivery(&mut self, ctx: &mut ActorContextGeneric<'_, TB>, delivery_id: u64) -> bool {
    let before = self.unconfirmed.len();
    self.unconfirmed.retain(|entry| entry.delivery_id() != delivery_id);
    if self.unconfirmed.is_empty() && self.redelivery_handle.is_some() {
      self.cancel_redelivery(ctx);
    }
    before != self.unconfirmed.len()
  }

  /// Returns a snapshot of the delivery state.
  #[must_use]
  pub fn get_delivery_snapshot(&self) -> AtLeastOnceDeliverySnapshot<TB> {
    AtLeastOnceDeliverySnapshot::new(self.current_delivery_id(), self.unconfirmed.clone())
  }

  /// Restores the delivery state from a snapshot.
  pub fn set_delivery_snapshot(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    snapshot: AtLeastOnceDeliverySnapshot<TB>,
  ) {
    let (current_id, unconfirmed) = snapshot.into_parts();
    self.next_delivery_id = current_id.saturating_add(1);
    self.unconfirmed = unconfirmed;
    if !self.unconfirmed.is_empty() {
      let _ = self.ensure_redelivery_scheduled(ctx);
    }
  }

  /// Handles internal redelivery messages.
  ///
  /// Returns `true` when the message was handled.
  ///
  /// # Errors
  ///
  /// Returns an error when redelivery fails.
  pub fn handle_message(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: &AnyMessageViewGeneric<'_, TB>,
  ) -> Result<bool, ActorError> {
    if message.downcast_ref::<RedeliveryTick>().is_some() {
      self.redeliver(ctx)?;
      return Ok(true);
    }
    Ok(false)
  }

  fn redeliver(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    if self.unconfirmed.is_empty() {
      self.cancel_redelivery(ctx);
      return Ok(());
    }
    let timestamp = now_instant(ctx);
    let burst_limit = self.config.redelivery_burst_limit().max(1);
    for entry in self.unconfirmed.iter_mut().take(burst_limit) {
      let payload = entry.payload_arc();
      let sender = entry.sender().cloned();
      let message = AnyMessageGeneric::from_erased(payload, sender);
      entry.destination().tell(message).map_err(|error| ActorError::from_send_error(&error))?;
      entry.mark_attempt(timestamp);
    }
    Ok(())
  }

  fn ensure_redelivery_scheduled(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) -> Result<(), ActorError> {
    if self.redelivery_handle.is_some() {
      return Ok(());
    }
    let interval = self.config.redeliver_interval();
    if interval == Duration::ZERO {
      return Err(ActorError::recoverable("redeliver interval must be positive"));
    }
    let receiver = ctx.self_ref();
    let message = AnyMessageGeneric::new(RedeliveryTick);
    let handle = ctx
      .system()
      .scheduler()
      .with_write(|scheduler| {
        scheduler.schedule_with_fixed_delay(interval, interval, SchedulerCommand::SendMessage {
          receiver,
          message,
          dispatcher: None,
          sender: None,
        })
      })
      .map_err(|error| map_scheduler_error(&error))?;
    self.redelivery_handle = Some(handle);
    Ok(())
  }

  fn cancel_redelivery(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) {
    if let Some(handle) = self.redelivery_handle.take() {
      ctx.system().scheduler().with_write(|scheduler| {
        scheduler.cancel(&handle);
      });
    }
  }
}

impl<TB: RuntimeToolbox + 'static> Default for AtLeastOnceDeliveryGeneric<TB> {
  fn default() -> Self {
    Self::new(AtLeastOnceDeliveryConfig::default())
  }
}

#[derive(Debug)]
struct RedeliveryTick;

fn now_instant<TB: RuntimeToolbox + 'static>(ctx: &ActorContextGeneric<'_, TB>) -> TimerInstant {
  let scheduler = ctx.system().scheduler();
  scheduler.with_read(|scheduler| scheduler.toolbox().clock().now())
}

fn map_scheduler_error(error: &SchedulerError) -> ActorError {
  ActorError::recoverable(format!("scheduler error: {error:?}"))
}
