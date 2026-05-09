//! Snapshot actor implementation.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::String, vec::Vec};
use core::{
  any::Any,
  future::Future,
  pin::Pin,
  task::{Context, Poll, Waker},
};

use fraktor_actor_core_rs::actor::{
  Actor, ActorContext,
  actor_ref::ActorRef,
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
};
use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::{
  snapshot::Snapshot, snapshot_actor_config::SnapshotActorConfig, snapshot_error::SnapshotError,
  snapshot_message::SnapshotMessage, snapshot_metadata::SnapshotMetadata, snapshot_response::SnapshotResponse,
  snapshot_selection_criteria::SnapshotSelectionCriteria, snapshot_store::SnapshotStore,
};

struct SnapshotPoll;

enum SnapshotInFlight {
  Save {
    future:      Pin<Box<dyn Future<Output = Result<(), SnapshotError>> + Send>>,
    metadata:    SnapshotMetadata,
    snapshot:    ArcShared<dyn Any + Send + Sync>,
    sender:      ActorRef,
    retry_count: u32,
  },
  Load {
    future:         Pin<Box<dyn Future<Output = Result<Option<Snapshot>, SnapshotError>> + Send>>,
    persistence_id: String,
    criteria:       SnapshotSelectionCriteria,
    sender:         ActorRef,
    retry_count:    u32,
  },
  DeleteOne {
    future:      Pin<Box<dyn Future<Output = Result<(), SnapshotError>> + Send>>,
    metadata:    SnapshotMetadata,
    sender:      ActorRef,
    retry_count: u32,
  },
  DeleteMany {
    future:         Pin<Box<dyn Future<Output = Result<(), SnapshotError>> + Send>>,
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
  match &mut entry {
    | SnapshotInFlight::Save { future, metadata, snapshot, sender, retry_count } => {
      match Future::poll(future.as_mut(), cx) {
        | Poll::Ready(Ok(())) => {
          sender
            .try_tell(AnyMessage::new(SnapshotResponse::SaveSnapshotSuccess { metadata: metadata.clone() }))
            .map_err(|error| ActorError::from_send_error(&error))?;
          Ok(None)
        },
        | Poll::Ready(Err(error)) => {
          if *retry_count < retry_max {
            *retry_count = retry_count.saturating_add(1);
            *future = Box::pin(snapshot_store.save_snapshot(metadata.clone(), snapshot.clone()));
            Ok(Some(entry))
          } else {
            sender
              .try_tell(AnyMessage::new(SnapshotResponse::SaveSnapshotFailure { metadata: metadata.clone(), error }))
              .map_err(|send_error| ActorError::from_send_error(&send_error))?;
            Ok(None)
          }
        },
        | Poll::Pending => Ok(Some(entry)),
      }
    },
    | SnapshotInFlight::Load { future, persistence_id, criteria, sender, retry_count } => {
      match Future::poll(future.as_mut(), cx) {
        | Poll::Ready(Ok(snapshot)) => {
          sender
            .try_tell(AnyMessage::new(SnapshotResponse::LoadSnapshotResult {
              snapshot,
              to_sequence_nr: criteria.max_sequence_nr(),
            }))
            .map_err(|error| ActorError::from_send_error(&error))?;
          Ok(None)
        },
        | Poll::Ready(Err(error)) => {
          if *retry_count < retry_max {
            *retry_count = retry_count.saturating_add(1);
            *future = Box::pin(snapshot_store.load_snapshot(persistence_id, criteria.clone()));
            Ok(Some(entry))
          } else {
            sender
              .try_tell(AnyMessage::new(SnapshotResponse::LoadSnapshotFailed { error }))
              .map_err(|send_error| ActorError::from_send_error(&send_error))?;
            Ok(None)
          }
        },
        | Poll::Pending => Ok(Some(entry)),
      }
    },
    | SnapshotInFlight::DeleteOne { future, metadata, sender, retry_count } => {
      match Future::poll(future.as_mut(), cx) {
        | Poll::Ready(Ok(())) => {
          sender
            .try_tell(AnyMessage::new(SnapshotResponse::DeleteSnapshotSuccess { metadata: metadata.clone() }))
            .map_err(|error| ActorError::from_send_error(&error))?;
          Ok(None)
        },
        | Poll::Ready(Err(error)) => {
          if *retry_count < retry_max {
            *retry_count = retry_count.saturating_add(1);
            *future = Box::pin(snapshot_store.delete_snapshot(metadata));
            Ok(Some(entry))
          } else {
            sender
              .try_tell(AnyMessage::new(SnapshotResponse::DeleteSnapshotFailure { metadata: metadata.clone(), error }))
              .map_err(|send_error| ActorError::from_send_error(&send_error))?;
            Ok(None)
          }
        },
        | Poll::Pending => Ok(Some(entry)),
      }
    },
    | SnapshotInFlight::DeleteMany { future, persistence_id, criteria, sender, retry_count } => {
      match Future::poll(future.as_mut(), cx) {
        | Poll::Ready(Ok(())) => {
          sender
            .try_tell(AnyMessage::new(SnapshotResponse::DeleteSnapshotsSuccess { criteria: criteria.clone() }))
            .map_err(|error| ActorError::from_send_error(&error))?;
          Ok(None)
        },
        | Poll::Ready(Err(error)) => {
          if *retry_count < retry_max {
            *retry_count = retry_count.saturating_add(1);
            *future = Box::pin(snapshot_store.delete_snapshots(persistence_id, criteria.clone()));
            Ok(Some(entry))
          } else {
            sender
              .try_tell(AnyMessage::new(SnapshotResponse::DeleteSnapshotsFailure { criteria: criteria.clone(), error }))
              .map_err(|send_error| ActorError::from_send_error(&send_error))?;
            Ok(None)
          }
        },
        | Poll::Pending => Ok(Some(entry)),
      }
    },
  }
}
