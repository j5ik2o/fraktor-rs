//! Snapshot actor implementation.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, string::String, vec::Vec};
use core::{
  future::Future,
  marker::PhantomData,
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use fraktor_actor_rs::core::{
  actor::{Actor, ActorContextGeneric, actor_ref::ActorRefGeneric},
  error::ActorError,
  messaging::{AnyMessageGeneric, AnyMessageViewGeneric},
};
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  snapshot::Snapshot, snapshot_actor_config::SnapshotActorConfig, snapshot_error::SnapshotError,
  snapshot_message::SnapshotMessage, snapshot_metadata::SnapshotMetadata, snapshot_response::SnapshotResponse,
  snapshot_selection_criteria::SnapshotSelectionCriteria, snapshot_store::SnapshotStore,
};

struct SnapshotPoll;

enum SnapshotInFlight<TB: RuntimeToolbox + 'static> {
  Save {
    future:      Pin<Box<dyn Future<Output = Result<(), SnapshotError>> + Send>>,
    metadata:    SnapshotMetadata,
    snapshot:    ArcShared<dyn core::any::Any + Send + Sync>,
    sender:      ActorRefGeneric<TB>,
    retry_count: u32,
  },
  Load {
    future:         Pin<Box<dyn Future<Output = Result<Option<Snapshot>, SnapshotError>> + Send>>,
    persistence_id: String,
    criteria:       SnapshotSelectionCriteria,
    sender:         ActorRefGeneric<TB>,
    retry_count:    u32,
  },
  DeleteOne {
    future:      Pin<Box<dyn Future<Output = Result<(), SnapshotError>> + Send>>,
    metadata:    SnapshotMetadata,
    sender:      ActorRefGeneric<TB>,
    retry_count: u32,
  },
  DeleteMany {
    future:         Pin<Box<dyn Future<Output = Result<(), SnapshotError>> + Send>>,
    persistence_id: String,
    criteria:       SnapshotSelectionCriteria,
    sender:         ActorRefGeneric<TB>,
    retry_count:    u32,
  },
}

/// Actor wrapper around a snapshot store implementation.
pub struct SnapshotActor<S: SnapshotStore, TB: RuntimeToolbox + 'static> {
  snapshot_store: S,
  in_flight:      Vec<SnapshotInFlight<TB>>,
  poll_scheduled: bool,
  config:         SnapshotActorConfig,
  _marker:        PhantomData<TB>,
}

impl<S: SnapshotStore, TB: RuntimeToolbox + 'static> SnapshotActor<S, TB>
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
    Self { snapshot_store, in_flight: Vec::new(), poll_scheduled: false, config, _marker: PhantomData }
  }

  fn schedule_poll(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) {
    if self.poll_scheduled || self.in_flight.is_empty() {
      return;
    }
    self.poll_scheduled = true;
    if ctx.self_ref().tell(AnyMessageGeneric::new(SnapshotPoll)).is_err() {
      // tell失敗時にフラグをリセットし、ポーリング停止を防ぐ
      self.poll_scheduled = false;
    }
  }

  fn poll_in_flight(&mut self, ctx: &mut ActorContextGeneric<'_, TB>) {
    self.poll_scheduled = false;
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut pending = Vec::new();
    let retry_max = self.config.retry_max();
    let in_flight = core::mem::take(&mut self.in_flight);
    for entry in in_flight {
      if let Some(entry) = poll_entry(&mut self.snapshot_store, entry, &mut cx, retry_max) {
        pending.push(entry);
      }
    }
    self.in_flight = pending;
    self.schedule_poll(ctx);
  }
}

impl<S: SnapshotStore, TB: RuntimeToolbox + 'static> Actor<TB> for SnapshotActor<S, TB>
where
  for<'a> S::SaveFuture<'a>: Send + 'static,
  for<'a> S::LoadFuture<'a>: Send + 'static,
  for<'a> S::DeleteOneFuture<'a>: Send + 'static,
  for<'a> S::DeleteManyFuture<'a>: Send + 'static,
{
  fn receive(
    &mut self,
    ctx: &mut ActorContextGeneric<'_, TB>,
    message: AnyMessageViewGeneric<'_, TB>,
  ) -> Result<(), ActorError> {
    if message.downcast_ref::<SnapshotPoll>().is_some() {
      self.poll_in_flight(ctx);
      return Ok(());
    }

    if let Some(msg) = message.downcast_ref::<SnapshotMessage<TB>>() {
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
      self.poll_in_flight(ctx);
    }
    Ok(())
  }
}

const fn noop_waker() -> Waker {
  const VTABLE: RawWakerVTable = RawWakerVTable::new(|data| RawWaker::new(data, &VTABLE), |_| {}, |_| {}, |_| {});

  const unsafe fn raw_waker() -> RawWaker {
    RawWaker::new(core::ptr::null(), &VTABLE)
  }

  unsafe { Waker::from_raw(raw_waker()) }
}

fn poll_entry<S: SnapshotStore, TB: RuntimeToolbox + 'static>(
  snapshot_store: &mut S,
  mut entry: SnapshotInFlight<TB>,
  cx: &mut Context<'_>,
  retry_max: u32,
) -> Option<SnapshotInFlight<TB>>
where
  for<'a> S::SaveFuture<'a>: Send + 'static,
  for<'a> S::LoadFuture<'a>: Send + 'static,
  for<'a> S::DeleteOneFuture<'a>: Send + 'static,
  for<'a> S::DeleteManyFuture<'a>: Send + 'static, {
  match &mut entry {
    | SnapshotInFlight::Save { future, metadata, snapshot, sender, retry_count } => {
      match Future::poll(future.as_mut(), cx) {
        | Poll::Ready(Ok(())) => {
          let _ =
            sender.tell(AnyMessageGeneric::new(SnapshotResponse::SaveSnapshotSuccess { metadata: metadata.clone() }));
          None
        },
        | Poll::Ready(Err(error)) => {
          if *retry_count < retry_max {
            *retry_count = retry_count.saturating_add(1);
            *future = Box::pin(snapshot_store.save_snapshot(metadata.clone(), snapshot.clone()));
            Some(entry)
          } else {
            let _ = sender.tell(AnyMessageGeneric::new(SnapshotResponse::SaveSnapshotFailure {
              metadata: metadata.clone(),
              error,
            }));
            None
          }
        },
        | Poll::Pending => Some(entry),
      }
    },
    | SnapshotInFlight::Load { future, persistence_id, criteria, sender, retry_count } => {
      match Future::poll(future.as_mut(), cx) {
        | Poll::Ready(Ok(snapshot)) => {
          let _ = sender.tell(AnyMessageGeneric::new(SnapshotResponse::LoadSnapshotResult {
            snapshot,
            to_sequence_nr: criteria.max_sequence_nr(),
          }));
          None
        },
        | Poll::Ready(Err(error)) => {
          if *retry_count < retry_max {
            *retry_count = retry_count.saturating_add(1);
            *future = Box::pin(snapshot_store.load_snapshot(persistence_id, criteria.clone()));
            Some(entry)
          } else {
            let _ = sender.tell(AnyMessageGeneric::new(SnapshotResponse::LoadSnapshotFailed { error }));
            None
          }
        },
        | Poll::Pending => Some(entry),
      }
    },
    | SnapshotInFlight::DeleteOne { future, metadata, sender, retry_count } => {
      match Future::poll(future.as_mut(), cx) {
        | Poll::Ready(Ok(())) => {
          let _ =
            sender.tell(AnyMessageGeneric::new(SnapshotResponse::DeleteSnapshotSuccess { metadata: metadata.clone() }));
          None
        },
        | Poll::Ready(Err(error)) => {
          if *retry_count < retry_max {
            *retry_count = retry_count.saturating_add(1);
            *future = Box::pin(snapshot_store.delete_snapshot(metadata));
            Some(entry)
          } else {
            let _ = sender.tell(AnyMessageGeneric::new(SnapshotResponse::DeleteSnapshotFailure {
              metadata: metadata.clone(),
              error,
            }));
            None
          }
        },
        | Poll::Pending => Some(entry),
      }
    },
    | SnapshotInFlight::DeleteMany { future, persistence_id, criteria, sender, retry_count } => {
      match Future::poll(future.as_mut(), cx) {
        | Poll::Ready(Ok(())) => {
          let _ = sender
            .tell(AnyMessageGeneric::new(SnapshotResponse::DeleteSnapshotsSuccess { criteria: criteria.clone() }));
          None
        },
        | Poll::Ready(Err(error)) => {
          if *retry_count < retry_max {
            *retry_count = retry_count.saturating_add(1);
            *future = Box::pin(snapshot_store.delete_snapshots(persistence_id, criteria.clone()));
            Some(entry)
          } else {
            let _ = sender.tell(AnyMessageGeneric::new(SnapshotResponse::DeleteSnapshotsFailure {
              criteria: criteria.clone(),
              error,
            }));
            None
          }
        },
        | Poll::Pending => Some(entry),
      }
    },
  }
}
