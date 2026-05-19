#[cfg(test)]
#[path = "stream_ref_endpoint_cleanup_test.rs"]
mod tests;

use alloc::string::ToString;

use fraktor_actor_core_kernel_rs::actor::{actor_ref::ActorRef, messaging::AnyMessage};

use super::stream_ref_endpoint_state::StreamRefEndpointState;
use crate::{StreamError, stage::StageActor, stream_ref::StreamRefRemoteStreamFailure};

const STREAM_REF_CANCELLATION_SIGNAL_MESSAGE: &str = "NoMoreElementsNeeded";

pub(crate) struct StreamRefEndpointCleanup {
  endpoint_actor: StageActor,
  partner_actor:  Option<ActorRef>,
}

impl StreamRefEndpointCleanup {
  /// Creates endpoint cleanup for a materialized StreamRef endpoint actor.
  pub(crate) const fn new(endpoint_actor: StageActor, partner_actor: Option<ActorRef>) -> Self {
    Self { endpoint_actor, partner_actor }
  }

  pub(crate) fn endpoint_actor_ref(&self) -> ActorRef {
    self.endpoint_actor.actor_ref().clone()
  }

  pub(crate) fn endpoint_actor(&self) -> StageActor {
    self.endpoint_actor.clone()
  }

  pub(crate) fn partner_actor(&self) -> Option<ActorRef> {
    self.partner_actor.clone()
  }

  pub(crate) fn set_partner_actor(&mut self, partner_actor: ActorRef) {
    self.partner_actor = Some(partner_actor);
  }

  pub(crate) fn run(self, state: &mut StreamRefEndpointState) -> Result<(), StreamError> {
    let terminal_signal_failure = self.send_partner_terminal_signal(state);
    let watch_release_failure = self.release_partner_watch(state);
    let shutdown_failure = self.shutdown_endpoint_actor(state);
    Self::combine_failures(terminal_signal_failure, watch_release_failure, shutdown_failure)
  }

  fn send_partner_terminal_signal(&self, state: &StreamRefEndpointState) -> Option<StreamError> {
    if !state.is_cancelled() {
      return None;
    }
    let mut partner_actor = self.partner_actor.clone()?;
    let signal = StreamRefRemoteStreamFailure::new(STREAM_REF_CANCELLATION_SIGNAL_MESSAGE.to_string());
    let message = AnyMessage::new(signal).with_sender(self.endpoint_actor.actor_ref().clone());
    partner_actor.try_tell(message).err().map(|error| StreamError::from_send_error(&error))
  }

  fn release_partner_watch(&self, state: &mut StreamRefEndpointState) -> Option<StreamError> {
    let partner_actor = self.partner_actor.as_ref()?;
    match self.endpoint_actor.unwatch(partner_actor) {
      | Ok(()) => None,
      | Err(error) => {
        state.record_watch_release_failure(error.clone());
        debug_assert_eq!(state.watch_release_failure(), Some(&error));
        Some(error)
      },
    }
  }

  fn shutdown_endpoint_actor(&self, state: &mut StreamRefEndpointState) -> Option<StreamError> {
    match self.endpoint_actor.stop() {
      | Ok(()) => None,
      | Err(error) => {
        state.record_shutdown_failure(error.clone());
        debug_assert_eq!(state.shutdown_failure(), Some(&error));
        Some(error)
      },
    }
  }

  fn combine_failures(
    terminal_signal_failure: Option<StreamError>,
    watch_release_failure: Option<StreamError>,
    shutdown_failure: Option<StreamError>,
  ) -> Result<(), StreamError> {
    match Self::merge_failure(Self::merge_failure(terminal_signal_failure, watch_release_failure), shutdown_failure) {
      | Some(error) => Err(error),
      | None => Ok(()),
    }
  }

  fn merge_failure(primary: Option<StreamError>, cleanup: Option<StreamError>) -> Option<StreamError> {
    match (primary, cleanup) {
      | (Some(primary), Some(cleanup)) => Some(StreamError::materialized_resource_rollback_failed(primary, cleanup)),
      | (Some(primary), None) => Some(primary),
      | (None, Some(cleanup)) => Some(cleanup),
      | (None, None) => None,
    }
  }
}
