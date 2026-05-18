use crate::{
  StreamError,
  stage::{StageActorEnvelope, StageActorReceive},
};

pub(crate) struct StreamRefTargetNotInitializedReceive;

impl StageActorReceive for StreamRefTargetNotInitializedReceive {
  fn receive(&mut self, _envelope: StageActorEnvelope) -> Result<(), StreamError> {
    Err(StreamError::StreamRefTargetNotInitialized)
  }
}
