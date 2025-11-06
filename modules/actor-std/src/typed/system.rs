use std::marker::PhantomData;

use cellactor_actor_core_rs::{
  spawn::SpawnError,
  system::ActorSystemGeneric,
  typed::{TypedActorSystemGeneric as CoreTypedActorSystemGeneric, TypedPropsGeneric},
};
use cellactor_actor_core_rs::actor_prim::Pid;
use cellactor_actor_core_rs::event_stream::EventStreamGeneric;
use cellactor_actor_core_rs::system::SystemStateGeneric;
use cellactor_actor_core_rs::typed::actor_prim::TypedActorRefGeneric;
use cellactor_utils_core_rs::ArcShared;
use cellactor_utils_std_rs::runtime_toolbox::StdToolbox;
use crate::event_stream::EventStream;
use crate::system::SystemState;
use crate::typed::actor_prim::TypedActorRef;
use crate::typed::TypedProps;

pub struct TypedActorSystem<M>
where
  M: Send + Sync + 'static, {
  inner: CoreTypedActorSystemGeneric<M, StdToolbox>,
}

impl<M> TypedActorSystem<M>
where
  M: Send + Sync + 'static,
{
  pub fn new_empty() -> Self {
    Self { inner: CoreTypedActorSystemGeneric::new_empty() }
  }

  pub fn new(guardian: &TypedProps<M>) -> Result<Self, SpawnError> {
    Ok(Self { inner: CoreTypedActorSystemGeneric::new(guardian)? })
  }

  /// Returns the typed user guardian reference.
  #[must_use]
  pub fn user_guardian_ref(&self) -> TypedActorRef<M> {
    self.inner.user_guardian_ref()
  }

  /// Returns the shared system state handle.
  #[must_use]
  pub fn state(&self) -> ArcShared<SystemState> {
    self.inner.state()
  }

  /// Allocates a new pid (testing helper).
  #[must_use]
  pub fn allocate_pid(&self) -> Pid {
    self.inner.allocate_pid()
  }

  /// Returns the shared event stream handle.
  #[must_use]
  pub fn event_stream(&self) -> ArcShared<EventStream> {
    self.inner.event_stream()
  }
}
