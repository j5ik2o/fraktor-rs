use fraktor_actor_core_kernel_rs::actor::message_adapter::{
  AdapterLifecycleState, AdapterRefHandleId, MessageAdapterRegistration,
};

fn main() {
  let _ = core::any::type_name::<AdapterLifecycleState>();
  let _ = core::any::type_name::<AdapterRefHandleId>();
  let _ = core::any::type_name::<MessageAdapterRegistration>();
}
