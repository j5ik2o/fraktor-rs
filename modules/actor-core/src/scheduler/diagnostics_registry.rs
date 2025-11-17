use alloc::{collections::VecDeque, vec::Vec};

use fraktor_utils_core_rs::core::sync::{ArcShared, NoStdMutex};

use super::SchedulerDiagnosticsEvent;

pub(crate) struct PublishOutcome {
  pub(crate) delivered: bool,
  pub(crate) dropped:   bool,
}

pub(crate) struct DiagnosticsSubscriber {
  pub(crate) id:     u64,
  pub(crate) buffer: ArcShared<DiagnosticsBuffer>,
}

pub(crate) struct DiagnosticsBuffer {
  queue:    NoStdMutex<VecDeque<SchedulerDiagnosticsEvent>>,
  capacity: usize,
}

impl DiagnosticsBuffer {
  pub(crate) const fn new(capacity: usize) -> Self {
    Self { queue: NoStdMutex::new(VecDeque::new()), capacity }
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
#[derive(Clone)]
pub(crate) struct DiagnosticsRegistry {
  entries: ArcShared<NoStdMutex<Vec<DiagnosticsSubscriber>>>,
}

impl DiagnosticsRegistry {
  pub(crate) fn new() -> Self {
    Self { entries: ArcShared::new(NoStdMutex::new(Vec::new())) }
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
