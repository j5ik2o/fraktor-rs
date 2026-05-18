use alloc::{format, string::String};

use fraktor_actor_core_kernel_rs::actor::{Pid, actor_ref::ActorRef, messaging::AnyMessage};

use super::StreamRefHandoff;
use crate::{
  StreamError,
  stream_ref::{StreamRefAck, StreamRefOnSubscribeHandshake},
};

pub(crate) struct StreamRefEndpointReceiveState<T> {
  handoff:            StreamRefHandoff<T>,
  endpoint_actor_ref: ActorRef,
  partner_actor:      Option<ActorRef>,
}

impl<T> StreamRefEndpointReceiveState<T>
where
  T: Send + Sync + 'static,
{
  pub(crate) const fn new(handoff: StreamRefHandoff<T>, endpoint_actor_ref: ActorRef) -> Self {
    Self { handoff, endpoint_actor_ref, partner_actor: None }
  }

  pub(crate) const fn handoff(&self) -> &StreamRefHandoff<T> {
    &self.handoff
  }

  pub(crate) fn stream_error_from_context(message: impl Into<String>) -> StreamError {
    StreamError::failed_with_context(message.into())
  }

  pub(crate) fn send_to_partner<M>(&mut self, message: M) -> Result<(), StreamError>
  where
    M: Send + Sync + 'static, {
    let Some(partner_actor) = &self.partner_actor else {
      return Err(StreamError::StreamRefTargetNotInitialized);
    };
    let mut partner_actor = partner_actor.clone();
    partner_actor
      .try_tell(AnyMessage::new(message).with_sender(self.endpoint_actor_ref.clone()))
      .map_err(|error| StreamError::from_send_error(&error))
  }

  pub(crate) fn accept_handshake(
    &mut self,
    message: &StreamRefOnSubscribeHandshake,
    sender: &ActorRef,
  ) -> Result<(), StreamError> {
    let partner_actor = sender.clone();
    self.handoff.pair_partner_actor(String::from(message.target_ref_path()), partner_actor.clone())?;
    self.partner_actor = Some(partner_actor);
    self.send_to_partner(StreamRefAck)
  }

  pub(crate) fn ensure_sender(&self, sender: &ActorRef) -> Result<(), StreamError> {
    self.handoff.ensure_partner_actor(sender)
  }

  pub(crate) fn accept_partner_terminated(&self, terminated: &Pid) -> Result<(), StreamError> {
    if self.handoff.is_terminal() {
      return Ok(());
    }
    let error = StreamError::RemoteStreamRefActorTerminated {
      message: format!("remote stream ref partner actor terminated: {terminated:?}").into(),
    };
    Err(self.handoff.fail_and_report(error))
  }
}
