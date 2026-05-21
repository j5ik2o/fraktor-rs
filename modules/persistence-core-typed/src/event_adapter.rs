//! Typed event adapter contract.

#[cfg(test)]
#[path = "event_adapter_test.rs"]
mod tests;

use alloc::string::String;
use core::{any::Any, ops::Deref};

use fraktor_persistence_core_kernel_rs::journal::{EventSeq as KernelEventSeq, ReadEventAdapter, WriteEventAdapter};
use fraktor_utils_core_rs::sync::ArcShared;

use crate::EventSeq;

/// Converts typed events to and from journal payloads.
pub trait EventAdapter<E>: Send + Sync + 'static {
  /// Returns the manifest associated with the event.
  fn manifest(&self, event: &E) -> String;

  /// Converts a typed event into a journal payload representation.
  fn to_journal(&self, event: E) -> ArcShared<dyn Any + Send + Sync>;

  /// Converts a journal payload and manifest back into typed events.
  fn adapt_from_journal(&self, event: ArcShared<dyn Any + Send + Sync>, manifest: &str) -> EventSeq<E>;
}

pub(crate) struct KernelEventAdapterBridge<E> {
  adapter: ArcShared<dyn EventAdapter<E>>,
}

impl<E> KernelEventAdapterBridge<E> {
  pub(crate) const fn new(adapter: ArcShared<dyn EventAdapter<E>>) -> Self {
    Self { adapter }
  }
}

impl<E> WriteEventAdapter for KernelEventAdapterBridge<E>
where
  E: Clone + Send + Sync + 'static,
{
  fn manifest(&self, event: &(dyn Any + Send + Sync)) -> String {
    event.downcast_ref::<E>().map_or_else(String::new, |typed_event| self.adapter.manifest(typed_event))
  }

  fn to_journal(&self, event: ArcShared<dyn Any + Send + Sync>) -> ArcShared<dyn Any + Send + Sync> {
    let Some(typed_event) = event.deref().downcast_ref::<E>() else {
      return event;
    };
    self.adapter.to_journal(typed_event.clone())
  }
}

impl<E> ReadEventAdapter for KernelEventAdapterBridge<E>
where
  E: Send + Sync + 'static,
{
  fn adapt_from_journal(&self, event: ArcShared<dyn Any + Send + Sync>, manifest: &str) -> KernelEventSeq {
    let events = self.adapter.adapt_from_journal(event, manifest).into_events();
    let payloads = events
      .into_iter()
      .map(|event| {
        let payload: ArcShared<dyn Any + Send + Sync> = ArcShared::new(event);
        payload
      })
      .collect();
    KernelEventSeq::multiple(payloads)
  }
}
