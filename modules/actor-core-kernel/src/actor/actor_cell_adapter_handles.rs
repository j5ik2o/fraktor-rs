//! Actor cell adapter handles facet for actor cells.

use fraktor_utils_core_rs::sync::ArcShared;

use crate::actor::{
  ActorCell,
  message_adapter::{AdapterLifecycleState, AdapterRefHandle, AdapterRefHandleId},
};

impl ActorCell {
  /// Allocates and tracks a new adapter handle for message adapters.
  pub(crate) fn acquire_adapter_handle(&self) -> (AdapterRefHandleId, ArcShared<AdapterLifecycleState>) {
    self.state.with_write(|state| {
      let id = state.adapter_handle_counter.wrapping_add(1);
      state.adapter_handle_counter = id;
      let handle_id = id;
      let lifecycle = ArcShared::new(AdapterLifecycleState::new());
      let handle = AdapterRefHandle::new(handle_id, lifecycle.clone());
      state.adapter_handles.push(handle);
      (handle_id, lifecycle)
    })
  }

  /// Removes the specified adapter handle and marks it as stopped.
  pub(crate) fn remove_adapter_handle(&self, handle_id: AdapterRefHandleId) {
    self.state.with_write(|state| {
      let handles = &mut state.adapter_handles;
      if let Some(index) = handles.iter().position(|handle| handle.id() == handle_id) {
        let handle = handles.remove(index);
        handle.stop();
      }
    });
  }

  /// Drops every tracked adapter handle, notifying senders that the actor stopped.
  pub(crate) fn drop_adapter_refs(&self) {
    self.state.with_write(|state| {
      for handle in state.adapter_handles.iter() {
        handle.stop();
      }
      state.adapter_handles.clear();
    });
  }
}
