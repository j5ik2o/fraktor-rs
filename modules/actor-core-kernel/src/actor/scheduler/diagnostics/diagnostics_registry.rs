use alloc::{collections::VecDeque, vec::Vec};
use core::marker::PhantomData;

use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedAccess, SharedLock};

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
  queue:    SharedLock<VecDeque<SchedulerDiagnosticsEvent>>,
  capacity: usize,
  _marker:  PhantomData<()>,
}

impl DiagnosticsBuffer {
  pub(crate) fn new(capacity: usize) -> Self {
    Self { queue: SharedLock::new_with_driver::<DefaultMutex<_>>(VecDeque::new()), capacity, _marker: PhantomData }
  }

  pub(crate) fn push(&self, event: &SchedulerDiagnosticsEvent) -> bool {
    self.queue.with_write(|guard| {
      let mut dropped = false;
      if guard.len() >= self.capacity {
        guard.pop_front();
        dropped = true;
      }
      guard.push_back(event.clone());
      dropped
    })
  }

  pub(crate) fn drain(&self) -> Vec<SchedulerDiagnosticsEvent> {
    self.queue.with_write(|guard| guard.drain(..).collect())
  }
}

/// Streams scheduler events to diagnostic subscribers.
pub(crate) struct DiagnosticsRegistry {
  entries: SharedLock<Vec<DiagnosticsSubscriber>>,
  _marker: PhantomData<()>,
}

impl Clone for DiagnosticsRegistry {
  fn clone(&self) -> Self {
    Self { entries: self.entries.clone(), _marker: PhantomData }
  }
}

impl DiagnosticsRegistry {
  pub(crate) fn new() -> Self {
    Self { entries: SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new()), _marker: PhantomData }
  }

  pub(crate) fn register(&self, id: u64, capacity: usize) -> ArcShared<DiagnosticsBuffer> {
    let buffer = ArcShared::new(DiagnosticsBuffer::new(capacity));
    self.entries.with_write(|entries| entries.push(DiagnosticsSubscriber { id, buffer: buffer.clone() }));
    buffer
  }

  pub(crate) fn remove(&self, id: u64) {
    self.entries.with_write(|entries| {
      if let Some(position) = entries.iter().position(|entry| entry.id == id) {
        entries.swap_remove(position);
      }
    });
  }

  pub(crate) fn publish(&self, event: &SchedulerDiagnosticsEvent) -> PublishOutcome {
    self.entries.with_read(|entries| {
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
    })
  }
}
