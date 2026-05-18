//! Snapshot actor implementation.

#[cfg(test)]
#[path = "snapshot_actor_test.rs"]
mod tests;

use alloc::{boxed::Box, string::String, vec::Vec};
use core::{
  any::Any,
  future::Future,
  pin::Pin,
  task::{Context, Poll, Waker},
};

use fraktor_actor_core_kernel_rs::actor::{
  Actor, ActorContext,
  actor_ref::ActorRef,
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::snapshot::{
  Snapshot, SnapshotActorConfig, SnapshotError, SnapshotMessage, SnapshotMetadata, SnapshotResponse,
  SnapshotSelectionCriteria, SnapshotStore,
};

struct SnapshotPoll;

type SnapshotSaveFuture = Pin<Box<dyn Future<Output = Result<(), SnapshotError>> + Send>>;
type SnapshotLoadFuture = Pin<Box<dyn Future<Output = Result<Option<Snapshot>, SnapshotError>> + Send>>;
type SnapshotDeleteFuture = Pin<Box<dyn Future<Output = Result<(), SnapshotError>> + Send>>;

struct SnapshotPollContext<'a, S: SnapshotStore> {
  snapshot_store: &'a mut S,
  retry_max:      u32,
}

enum SnapshotInFlight {
  Save {
    future:      SnapshotSaveFuture,
    metadata:    SnapshotMetadata,
    snapshot:    ArcShared<dyn Any + Send + Sync>,
    sender:      ActorRef,
    retry_count: u32,
  },
  Load {
    future:         SnapshotLoadFuture,
    persistence_id: String,
    criteria:       SnapshotSelectionCriteria,
    sender:         ActorRef,
    retry_count:    u32,
  },
  DeleteOne {
    future:      SnapshotDeleteFuture,
    metadata:    SnapshotMetadata,
    sender:      ActorRef,
    retry_count: u32,
  },
  DeleteMany {
    future:         SnapshotDeleteFuture,
    persistence_id: String,
    criteria:       SnapshotSelectionCriteria,
    sender:         ActorRef,
    retry_count:    u32,
  },
}

/// Actor wrapper around a snapshot store implementation.
pub struct SnapshotActor<S: SnapshotStore> {
  snapshot_store: S,
  in_flight:      Vec<SnapshotInFlight>,
  poll_scheduled: bool,
  config:         SnapshotActorConfig,
}

impl<S: SnapshotStore> SnapshotActor<S>
where
  for<'a> S::SaveFuture<'a>: Send + 'static,
  for<'a> S::LoadFuture<'a>: Send + 'static,
  for<'a> S::DeleteOneFuture<'a>: Send + 'static,
  for<'a> S::DeleteManyFuture<'a>: Send + 'static,
{
  /// Creates a new snapshot actor.
  #[must_use]
  pub const fn new(snapshot_store: S) -> Self {
    Self::new_with_config(snapshot_store, SnapshotActorConfig::default_config())
  }

  /// Creates a new snapshot actor with configuration.
  #[must_use]
  pub const fn new_with_config(snapshot_store: S, config: SnapshotActorConfig) -> Self {
    Self { snapshot_store, in_flight: Vec::new(), poll_scheduled: false, config }
  }

  fn schedule_poll(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    if self.poll_scheduled || self.in_flight.is_empty() {
      return Ok(());
    }
    self.poll_scheduled = true;
    ctx.self_ref().try_tell(AnyMessage::new(SnapshotPoll)).map_err(|error| ActorError::from_send_error(&error))
  }

  fn poll_in_flight(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.poll_scheduled = false;
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    let mut pending = Vec::new();
    let retry_max = self.config.retry_max();
    let in_flight = core::mem::take(&mut self.in_flight);
    for entry in in_flight {
      if let Some(entry) = poll_entry(&mut self.snapshot_store, entry, &mut cx, retry_max)? {
        pending.push(entry);
      }
    }
    self.in_flight = pending;
    self.schedule_poll(ctx)
  }
}

impl<S: SnapshotStore> Actor for SnapshotActor<S>
where
  for<'a> S::SaveFuture<'a>: Send + 'static,
  for<'a> S::LoadFuture<'a>: Send + 'static,
  for<'a> S::DeleteOneFuture<'a>: Send + 'static,
  for<'a> S::DeleteManyFuture<'a>: Send + 'static,
{
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<SnapshotPoll>().is_some() {
      self.poll_in_flight(ctx)?;
      return Ok(());
    }

    if let Some(msg) = message.downcast_ref::<SnapshotMessage>() {
      match msg {
        | SnapshotMessage::SaveSnapshot { metadata, snapshot, sender } => {
          let future = Box::pin(self.snapshot_store.save_snapshot(metadata.clone(), snapshot.clone()));
          self.in_flight.push(SnapshotInFlight::Save {
            future,
            metadata: metadata.clone(),
            snapshot: snapshot.clone(),
            sender: sender.clone(),
            retry_count: 0,
          });
        },
        | SnapshotMessage::LoadSnapshot { persistence_id, criteria, sender } => {
          let future = Box::pin(self.snapshot_store.load_snapshot(persistence_id, criteria.clone()));
          self.in_flight.push(SnapshotInFlight::Load {
            future,
            persistence_id: persistence_id.clone(),
            criteria: criteria.clone(),
            sender: sender.clone(),
            retry_count: 0,
          });
        },
        | SnapshotMessage::DeleteSnapshot { metadata, sender } => {
          let future = Box::pin(self.snapshot_store.delete_snapshot(metadata));
          self.in_flight.push(SnapshotInFlight::DeleteOne {
            future,
            metadata: metadata.clone(),
            sender: sender.clone(),
            retry_count: 0,
          });
        },
        | SnapshotMessage::DeleteSnapshots { persistence_id, criteria, sender } => {
          let future = Box::pin(self.snapshot_store.delete_snapshots(persistence_id, criteria.clone()));
          self.in_flight.push(SnapshotInFlight::DeleteMany {
            future,
            persistence_id: persistence_id.clone(),
            criteria: criteria.clone(),
            sender: sender.clone(),
            retry_count: 0,
          });
        },
      }
      self.poll_in_flight(ctx)?;
    }
    Ok(())
  }
}

/// Returns `Some(entry)` when the in-flight operation is still pending.
fn poll_entry<S: SnapshotStore>(
  snapshot_store: &mut S,
  mut entry: SnapshotInFlight,
  cx: &mut Context<'_>,
  retry_max: u32,
) -> Result<Option<SnapshotInFlight>, ActorError>
where
  for<'a> S::SaveFuture<'a>: Send + 'static,
  for<'a> S::LoadFuture<'a>: Send + 'static,
  for<'a> S::DeleteOneFuture<'a>: Send + 'static,
  for<'a> S::DeleteManyFuture<'a>: Send + 'static, {
  let mut poll_context = SnapshotPollContext { snapshot_store, retry_max };
  let keep_pending = match &mut entry {
    | SnapshotInFlight::Save { future, metadata, snapshot, sender, retry_count } => {
      poll_save_entry(&mut poll_context, cx, future, metadata, snapshot, sender, retry_count)?
    },
    | SnapshotInFlight::Load { future, persistence_id, criteria, sender, retry_count } => {
      poll_load_entry(&mut poll_context, cx, future, persistence_id, criteria, sender, retry_count)?
    },
    | SnapshotInFlight::DeleteOne { future, metadata, sender, retry_count } => {
      poll_delete_one_entry(&mut poll_context, cx, future, metadata, sender, retry_count)?
    },
    | SnapshotInFlight::DeleteMany { future, persistence_id, criteria, sender, retry_count } => {
      poll_delete_many_entry(&mut poll_context, cx, future, persistence_id, criteria, sender, retry_count)?
    },
  };

  if keep_pending { Ok(Some(entry)) } else { Ok(None) }
}

fn poll_save_entry<S: SnapshotStore>(
  poll_context: &mut SnapshotPollContext<'_, S>,
  cx: &mut Context<'_>,
  future: &mut SnapshotSaveFuture,
  metadata: &SnapshotMetadata,
  snapshot: &ArcShared<dyn Any + Send + Sync>,
  sender: &mut ActorRef,
  retry_count: &mut u32,
) -> Result<bool, ActorError>
where
  for<'a> S::SaveFuture<'a>: Send + 'static, {
  match Future::poll(future.as_mut(), cx) {
    | Poll::Ready(Ok(())) => send_save_success(sender, metadata),
    | Poll::Ready(Err(error)) => {
      retry_or_fail_save(poll_context, future, metadata, snapshot, sender, retry_count, error)
    },
    | Poll::Pending => Ok(true),
  }
}

fn send_save_success(sender: &mut ActorRef, metadata: &SnapshotMetadata) -> Result<bool, ActorError> {
  sender
    .try_tell(AnyMessage::new(SnapshotResponse::SaveSnapshotSuccess { metadata: metadata.clone() }))
    .map_err(|error| ActorError::from_send_error(&error))?;
  Ok(false)
}

fn retry_or_fail_save<S: SnapshotStore>(
  poll_context: &mut SnapshotPollContext<'_, S>,
  future: &mut SnapshotSaveFuture,
  metadata: &SnapshotMetadata,
  snapshot: &ArcShared<dyn Any + Send + Sync>,
  sender: &mut ActorRef,
  retry_count: &mut u32,
  error: SnapshotError,
) -> Result<bool, ActorError>
where
  for<'a> S::SaveFuture<'a>: Send + 'static, {
  if *retry_count < poll_context.retry_max {
    *retry_count = retry_count.saturating_add(1);
    *future = Box::pin(poll_context.snapshot_store.save_snapshot(metadata.clone(), snapshot.clone()));
    return Ok(true);
  }
  sender
    .try_tell(AnyMessage::new(SnapshotResponse::SaveSnapshotFailure { metadata: metadata.clone(), error }))
    .map_err(|send_error| ActorError::from_send_error(&send_error))?;
  Ok(false)
}

fn poll_load_entry<S: SnapshotStore>(
  poll_context: &mut SnapshotPollContext<'_, S>,
  cx: &mut Context<'_>,
  future: &mut SnapshotLoadFuture,
  persistence_id: &str,
  criteria: &SnapshotSelectionCriteria,
  sender: &mut ActorRef,
  retry_count: &mut u32,
) -> Result<bool, ActorError>
where
  for<'a> S::LoadFuture<'a>: Send + 'static, {
  match Future::poll(future.as_mut(), cx) {
    | Poll::Ready(Ok(snapshot)) => send_load_success(sender, snapshot, criteria.max_sequence_nr()),
    | Poll::Ready(Err(error)) => {
      retry_or_fail_load(poll_context, future, persistence_id, criteria, sender, retry_count, error)
    },
    | Poll::Pending => Ok(true),
  }
}

fn send_load_success(
  sender: &mut ActorRef,
  snapshot: Option<Snapshot>,
  to_sequence_nr: u64,
) -> Result<bool, ActorError> {
  sender
    .try_tell(AnyMessage::new(SnapshotResponse::LoadSnapshotResult { snapshot, to_sequence_nr }))
    .map_err(|error| ActorError::from_send_error(&error))?;
  Ok(false)
}

fn retry_or_fail_load<S: SnapshotStore>(
  poll_context: &mut SnapshotPollContext<'_, S>,
  future: &mut SnapshotLoadFuture,
  persistence_id: &str,
  criteria: &SnapshotSelectionCriteria,
  sender: &mut ActorRef,
  retry_count: &mut u32,
  error: SnapshotError,
) -> Result<bool, ActorError>
where
  for<'a> S::LoadFuture<'a>: Send + 'static, {
  if *retry_count < poll_context.retry_max {
    *retry_count = retry_count.saturating_add(1);
    *future = Box::pin(poll_context.snapshot_store.load_snapshot(persistence_id, criteria.clone()));
    return Ok(true);
  }
  sender
    .try_tell(AnyMessage::new(SnapshotResponse::LoadSnapshotFailed { error }))
    .map_err(|send_error| ActorError::from_send_error(&send_error))?;
  Ok(false)
}

fn poll_delete_one_entry<S: SnapshotStore>(
  poll_context: &mut SnapshotPollContext<'_, S>,
  cx: &mut Context<'_>,
  future: &mut SnapshotDeleteFuture,
  metadata: &SnapshotMetadata,
  sender: &mut ActorRef,
  retry_count: &mut u32,
) -> Result<bool, ActorError>
where
  for<'a> S::DeleteOneFuture<'a>: Send + 'static, {
  match Future::poll(future.as_mut(), cx) {
    | Poll::Ready(Ok(())) => send_delete_one_success(sender, metadata),
    | Poll::Ready(Err(error)) => retry_or_fail_delete_one(poll_context, future, metadata, sender, retry_count, error),
    | Poll::Pending => Ok(true),
  }
}

fn send_delete_one_success(sender: &mut ActorRef, metadata: &SnapshotMetadata) -> Result<bool, ActorError> {
  sender
    .try_tell(AnyMessage::new(SnapshotResponse::DeleteSnapshotSuccess { metadata: metadata.clone() }))
    .map_err(|error| ActorError::from_send_error(&error))?;
  Ok(false)
}

fn retry_or_fail_delete_one<S: SnapshotStore>(
  poll_context: &mut SnapshotPollContext<'_, S>,
  future: &mut SnapshotDeleteFuture,
  metadata: &SnapshotMetadata,
  sender: &mut ActorRef,
  retry_count: &mut u32,
  error: SnapshotError,
) -> Result<bool, ActorError>
where
  for<'a> S::DeleteOneFuture<'a>: Send + 'static, {
  if *retry_count < poll_context.retry_max {
    *retry_count = retry_count.saturating_add(1);
    *future = Box::pin(poll_context.snapshot_store.delete_snapshot(metadata));
    return Ok(true);
  }
  sender
    .try_tell(AnyMessage::new(SnapshotResponse::DeleteSnapshotFailure { metadata: metadata.clone(), error }))
    .map_err(|send_error| ActorError::from_send_error(&send_error))?;
  Ok(false)
}

fn poll_delete_many_entry<S: SnapshotStore>(
  poll_context: &mut SnapshotPollContext<'_, S>,
  cx: &mut Context<'_>,
  future: &mut SnapshotDeleteFuture,
  persistence_id: &str,
  criteria: &SnapshotSelectionCriteria,
  sender: &mut ActorRef,
  retry_count: &mut u32,
) -> Result<bool, ActorError>
where
  for<'a> S::DeleteManyFuture<'a>: Send + 'static, {
  match Future::poll(future.as_mut(), cx) {
    | Poll::Ready(Ok(())) => send_delete_many_success(sender, criteria),
    | Poll::Ready(Err(error)) => {
      retry_or_fail_delete_many(poll_context, future, persistence_id, criteria, sender, retry_count, error)
    },
    | Poll::Pending => Ok(true),
  }
}

fn send_delete_many_success(sender: &mut ActorRef, criteria: &SnapshotSelectionCriteria) -> Result<bool, ActorError> {
  sender
    .try_tell(AnyMessage::new(SnapshotResponse::DeleteSnapshotsSuccess { criteria: criteria.clone() }))
    .map_err(|error| ActorError::from_send_error(&error))?;
  Ok(false)
}

fn retry_or_fail_delete_many<S: SnapshotStore>(
  poll_context: &mut SnapshotPollContext<'_, S>,
  future: &mut SnapshotDeleteFuture,
  persistence_id: &str,
  criteria: &SnapshotSelectionCriteria,
  sender: &mut ActorRef,
  retry_count: &mut u32,
  error: SnapshotError,
) -> Result<bool, ActorError>
where
  for<'a> S::DeleteManyFuture<'a>: Send + 'static, {
  if *retry_count < poll_context.retry_max {
    *retry_count = retry_count.saturating_add(1);
    *future = Box::pin(poll_context.snapshot_store.delete_snapshots(persistence_id, criteria.clone()));
    return Ok(true);
  }
  sender
    .try_tell(AnyMessage::new(SnapshotResponse::DeleteSnapshotsFailure { criteria: criteria.clone(), error }))
    .map_err(|send_error| ActorError::from_send_error(&send_error))?;
  Ok(false)
}
