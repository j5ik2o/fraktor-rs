use alloc::{collections::VecDeque, vec::Vec};
use core::marker::PhantomData;

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeMutex, sync::ArcShared};

use super::SchedulerDiagnosticsEvent;

pub(crate) struct PublishOutcome {
  pub(crate) delivered: bool,
  pub(crate) dropped:   bool,
}

pub(crate) struct DiagnosticsSubscriber {
  pub(crate) id:     u64,
  pub(crate) buffer: ArcShared<DiagnosticsBuffer>,
}
#[allow(dead_code)]
pub(crate) struct DiagnosticsBuffer {
  queue:    RuntimeMutex<VecDeque<SchedulerDiagnosticsEvent>>,
  capacity: usize,
  _marker:  PhantomData<()>,
}
#[allow(dead_code)]
impl DiagnosticsBuffer {
  pub(crate) const fn new(capacity: usize) -> Self {
    Self { queue: RuntimeMutex::new(VecDeque::new()), capacity, _marker: PhantomData }
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
pub(crate) struct DiagnosticsRegistry {
  entries: ArcShared<RuntimeMutex<Vec<DiagnosticsSubscriber>>>,
  _marker: PhantomData<()>,
}

impl Clone for DiagnosticsRegistry {
  fn clone(&self) -> Self {
    Self { entries: self.entries.clone(), _marker: PhantomData }
  }
}
#[allow(dead_code)]
impl DiagnosticsRegistry {
  pub(crate) fn new() -> Self {
    Self { entries: ArcShared::new(RuntimeMutex::new(Vec::new())), _marker: PhantomData }
  }

  pub(crate) fn register(&self, id: u64, capacity: usize) -> ArcShared<DiagnosticsBuffer> {
    let buffer = ArcShared::new(DiagnosticsBuffer::new(capacity));
    let mut entries = self.entries.lock();
    entries.push(DiagnosticsSubscriber { id, buffer: buffer.clone() });
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
