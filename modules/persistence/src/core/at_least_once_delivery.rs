//! At-least-once delivery helper.

#[cfg(test)]
mod tests;

use alloc::{format, vec::Vec};
use core::any::Any;

use fraktor_actor_rs::core::{actor::actor_ref::ActorRefGeneric, messaging::AnyMessageGeneric};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared, time::TimerInstant};

use crate::core::{
  at_least_once_delivery_config::AtLeastOnceDeliveryConfig,
  at_least_once_delivery_snapshot::AtLeastOnceDeliverySnapshot, persistence_error::PersistenceError,
  redelivery_tick::RedeliveryTick, unconfirmed_delivery::UnconfirmedDelivery, unconfirmed_warning::UnconfirmedWarning,
};

/// At-least-once delivery implementation.
pub struct AtLeastOnceDeliveryGeneric<TB: RuntimeToolbox + 'static> {
  config:          AtLeastOnceDeliveryConfig,
  delivery_seq_nr: u64,
  unconfirmed:     Vec<UnconfirmedDelivery<TB>>,
}

/// Type alias using default naming.
pub type AtLeastOnceDelivery<TB> = AtLeastOnceDeliveryGeneric<TB>;

impl<TB: RuntimeToolbox + 'static> AtLeastOnceDeliveryGeneric<TB> {
  /// Creates a new delivery tracker.
  #[must_use]
  pub const fn new(config: AtLeastOnceDeliveryConfig) -> Self {
    Self { config, delivery_seq_nr: 1, unconfirmed: Vec::new() }
  }

  /// Returns the configuration.
  #[must_use]
  pub const fn config(&self) -> &AtLeastOnceDeliveryConfig {
    &self.config
  }

  /// Returns the next delivery id without advancing.
  #[must_use]
  pub const fn current_delivery_id(&self) -> u64 {
    self.delivery_seq_nr
  }

  /// Returns the number of unconfirmed deliveries.
  #[must_use]
  pub const fn number_of_unconfirmed(&self) -> usize {
    self.unconfirmed.len()
  }

  /// Returns true if another delivery can be accepted.
  #[must_use]
  pub const fn can_accept_more(&self) -> bool {
    self.unconfirmed.len() < self.config.max_unconfirmed()
  }

  /// Allocates the next delivery id.
  #[must_use]
  pub const fn next_delivery_id(&mut self) -> u64 {
    let id = self.delivery_seq_nr;
    self.delivery_seq_nr = self.delivery_seq_nr.saturating_add(1);
    id
  }

  /// Adds an unconfirmed delivery.
  pub fn add_unconfirmed(&mut self, delivery: UnconfirmedDelivery<TB>) {
    self.unconfirmed.push(delivery);
  }

  /// Confirms a delivery by id.
  pub fn confirm_delivery(&mut self, delivery_id: u64) -> bool {
    if let Some(index) = self.unconfirmed.iter().position(|entry| entry.delivery_id() == delivery_id) {
      self.unconfirmed.remove(index);
      true
    } else {
      false
    }
  }

  /// Returns the unconfirmed deliveries.
  #[must_use]
  pub fn unconfirmed_deliveries(&self) -> &[UnconfirmedDelivery<TB>] {
    &self.unconfirmed
  }

  /// Returns deliveries that should be redelivered according to overdue deadline and burst limit.
  #[must_use]
  pub fn deliveries_to_redeliver(&self, now: TimerInstant) -> Vec<UnconfirmedDelivery<TB>> {
    let redeliver_interval = self.config.redeliver_interval();
    let limit = self.config.redelivery_burst_limit().min(self.unconfirmed.len());
    self
      .unconfirmed
      .iter()
      .filter(|delivery| Self::is_overdue(delivery, now, redeliver_interval))
      .take(limit)
      .cloned()
      .collect()
  }

  /// Returns a snapshot of current delivery state.
  #[must_use]
  pub fn get_delivery_snapshot(&self) -> AtLeastOnceDeliverySnapshot<TB> {
    AtLeastOnceDeliverySnapshot::new(self.delivery_seq_nr, self.unconfirmed.clone())
  }

  /// Restores delivery state from a snapshot.
  ///
  /// Restored entries are rebuilt so that redelivery attempts restart from zero
  /// and the next redelivery tick can resend them.
  pub fn set_delivery_snapshot(&mut self, snapshot: AtLeastOnceDeliverySnapshot<TB>, now: TimerInstant) {
    self.delivery_seq_nr = snapshot.current_delivery_id();
    let redelivery_base_timestamp = Self::redelivery_base_timestamp(now, self.config.redeliver_interval());
    self.unconfirmed = snapshot
      .into_unconfirmed()
      .into_iter()
      .map(|delivery| {
        UnconfirmedDelivery::new(
          delivery.delivery_id(),
          delivery.destination().clone(),
          delivery.payload_arc(),
          delivery.sender().cloned(),
          redelivery_base_timestamp,
          0,
        )
      })
      .collect();
  }

  /// Returns true when the message is a redelivery tick.
  #[must_use]
  pub fn is_redelivery_tick(message: &dyn Any) -> bool {
    message.is::<RedeliveryTick>()
  }

  /// Handles a redelivery tick message and returns a warning payload when the threshold is reached.
  /// # Errors
  ///
  /// Returns `PersistenceError::MessagePassing` when any redelivery send fails.
  pub fn handle_message(
    &mut self,
    message: &dyn Any,
    now: TimerInstant,
  ) -> Result<Option<UnconfirmedWarning<TB>>, PersistenceError> {
    if !Self::is_redelivery_tick(message) {
      return Ok(None);
    }
    self.redeliver_overdue(now)
  }

  fn redeliver_overdue(&mut self, now: TimerInstant) -> Result<Option<UnconfirmedWarning<TB>>, PersistenceError> {
    let mut warnings = Vec::new();
    let redeliver_interval = self.config.redeliver_interval();
    let warning_attempt = self.config.warn_after_number_of_unconfirmed_attempts();
    let burst_limit = self.config.redelivery_burst_limit();

    for delivery in self
      .unconfirmed
      .iter_mut()
      .filter(|delivery| Self::is_overdue(delivery, now, redeliver_interval))
      .take(burst_limit)
    {
      let warning = (delivery.attempt() == warning_attempt).then(|| delivery.clone());
      Self::send_delivery(delivery)?;
      delivery.mark_redelivered(now);
      if let Some(warning) = warning {
        warnings.push(warning);
      }
    }

    if warnings.is_empty() { Ok(None) } else { Ok(Some(UnconfirmedWarning::new(warnings))) }
  }

  fn is_overdue(
    delivery: &UnconfirmedDelivery<TB>,
    now: TimerInstant,
    redeliver_interval: core::time::Duration,
  ) -> bool {
    if delivery.attempt() == 0 {
      return true;
    }

    let now_nanos = Self::instant_to_nanos(now);
    let delivery_nanos = Self::instant_to_nanos(delivery.timestamp());
    let elapsed_nanos = now_nanos.saturating_sub(delivery_nanos);
    elapsed_nanos >= redeliver_interval.as_nanos()
  }

  fn instant_to_nanos(instant: TimerInstant) -> u128 {
    let tick_nanos = instant.resolution().as_nanos().max(1);
    u128::from(instant.ticks()).saturating_mul(tick_nanos)
  }

  fn redelivery_base_timestamp(now: TimerInstant, redeliver_interval: core::time::Duration) -> TimerInstant {
    let ticks = Self::duration_to_ticks(redeliver_interval, now.resolution());
    TimerInstant::from_ticks(now.ticks().saturating_sub(ticks), now.resolution())
  }

  fn duration_to_ticks(duration: core::time::Duration, resolution: core::time::Duration) -> u64 {
    let resolution_nanos = resolution.as_nanos().max(1);
    let ticks = duration.as_nanos().div_ceil(resolution_nanos);
    ticks.min(u128::from(u64::MAX)) as u64
  }

  /// Sends a tracked delivery and returns its id.
  ///
  /// # Errors
  ///
  /// Returns `PersistenceError::MessagePassing` when the delivery limit is exceeded
  /// or when the destination rejects the message.
  pub fn deliver<M>(
    &mut self,
    destination: ActorRefGeneric<TB>,
    sender: Option<ActorRefGeneric<TB>>,
    timestamp: TimerInstant,
    build: impl FnOnce(u64) -> M,
  ) -> Result<u64, PersistenceError>
  where
    M: Any + Send + Sync + 'static, {
    if !self.can_accept_more() {
      return Err(PersistenceError::MessagePassing("max unconfirmed deliveries exceeded".into()));
    }

    let delivery_id = self.next_delivery_id();
    let payload = ArcShared::new(build(delivery_id));
    let message = AnyMessageGeneric::from_erased(payload.clone(), sender.clone());
    destination.tell(message).map_err(|error| PersistenceError::MessagePassing(format!("{error:?}")))?;

    let unconfirmed = UnconfirmedDelivery::new(delivery_id, destination, payload, sender, timestamp, 1);
    self.add_unconfirmed(unconfirmed);
    Ok(delivery_id)
  }

  fn send_delivery(delivery: &UnconfirmedDelivery<TB>) -> Result<(), PersistenceError> {
    let message = AnyMessageGeneric::from_erased(delivery.payload_arc(), delivery.sender().cloned());
    delivery.destination().tell(message).map_err(|error| PersistenceError::MessagePassing(format!("{error:?}")))?;
    Ok(())
  }
}
