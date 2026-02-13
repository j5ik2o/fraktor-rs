use alloc::{collections::VecDeque, vec::Vec};
use core::marker::PhantomData;

use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::SchedulerDiagnosticsEvent;

pub(crate) struct PublishOutcome {
  pub(crate) delivered: bool,
  pub(crate) dropped:   bool,
}

pub(crate) struct DiagnosticsSubscriberGeneric<TB: RuntimeToolbox + 'static> {
  pub(crate) id:     u64,
  pub(crate) buffer: ArcShared<DiagnosticsBufferGeneric<TB>>,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type DiagnosticsSubscriber = DiagnosticsSubscriberGeneric<NoStdToolbox>;

pub(crate) struct DiagnosticsBufferGeneric<TB: RuntimeToolbox + 'static> {
  queue:    ToolboxMutex<VecDeque<SchedulerDiagnosticsEvent>, TB>,
  capacity: usize,
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type DiagnosticsBuffer = DiagnosticsBufferGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> DiagnosticsBufferGeneric<TB> {
  pub(crate) fn new(capacity: usize) -> Self {
    Self { queue: <TB::MutexFamily as SyncMutexFamily>::create(VecDeque::new()), capacity }
  }

  pub(crate) fn push(&self, event: &SchedulerDiagnosticsEvent) -> bool {
    let mut guard = self.queue.lock();
    let mut dropped = false;
    if guard.len() >= self.capacity {
      guard.pop_front();
      dropped = true;
    }
    guard.push_back(event.clone());
    dropped
  }

  pub(crate) fn drain(&self) -> Vec<SchedulerDiagnosticsEvent> {
    let mut guard = self.queue.lock();
    guard.drain(..).collect()
  }
}

/// Streams scheduler events to diagnostic subscribers.
pub(crate) struct DiagnosticsRegistryGeneric<TB: RuntimeToolbox + 'static> {
  entries: ArcShared<ToolboxMutex<Vec<DiagnosticsSubscriberGeneric<TB>>, TB>>,
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> Clone for DiagnosticsRegistryGeneric<TB> {
  fn clone(&self) -> Self {
    Self { entries: self.entries.clone(), _marker: PhantomData }
  }
}

/// Type alias using the default toolbox.
#[allow(dead_code)]
pub(crate) type DiagnosticsRegistry = DiagnosticsRegistryGeneric<NoStdToolbox>;

impl<TB: RuntimeToolbox + 'static> DiagnosticsRegistryGeneric<TB> {
  pub(crate) fn new() -> Self {
    Self { entries: ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(Vec::new())), _marker: PhantomData }
  }

  pub(crate) fn register(&self, id: u64, capacity: usize) -> ArcShared<DiagnosticsBufferGeneric<TB>> {
    let buffer = ArcShared::new(DiagnosticsBufferGeneric::new(capacity));
    let mut entries = self.entries.lock();
    entries.push(DiagnosticsSubscriberGeneric { id, buffer: buffer.clone() });
    buffer
  }

  pub(crate) fn remove(&self, id: u64) {
    let mut entries = self.entries.lock();
    if let Some(position) = entries.iter().position(|entry| entry.id == id) {
      entries.swap_remove(position);
    }
  }

  pub(crate) fn publish(&self, event: &SchedulerDiagnosticsEvent) -> PublishOutcome {
    let entries = self.entries.lock();
    if entries.is_empty() {
      return PublishOutcome { delivered: false, dropped: false };
    }
    let mut dropped = false;
    for entry in entries.iter() {
      if entry.buffer.push(event) {
        dropped = true;
      }
    }
    PublishOutcome { delivered: true, dropped }
  }
}
