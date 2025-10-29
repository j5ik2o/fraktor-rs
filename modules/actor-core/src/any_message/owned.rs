use core::{
  any::{Any, TypeId},
  fmt,
};

use cellactor_utils_core_rs::ArcShared;

use crate::{
  actor_ref::ActorRef,
  any_message::{AnyMessage, MessageMetadata},
};

/// Owned dynamic message stored inside mailboxes.
#[derive(Clone)]
pub struct AnyOwnedMessage {
  payload:  ArcShared<dyn Any + Send + Sync + 'static>,
  type_id:  TypeId,
  metadata: Option<MessageMetadata>,
  reply_to: Option<ActorRef>,
}

impl AnyOwnedMessage {
  /// Creates a new owned message from the provided payload.
  #[must_use]
  pub fn new<T>(payload: T) -> Self
  where
    T: Any + Send + Sync + 'static, {
    let type_id = TypeId::of::<T>();
    let payload = ArcShared::new(payload);
    let payload: ArcShared<dyn Any + Send + Sync + 'static> = payload;
    Self { payload, type_id, metadata: None, reply_to: None }
  }

  /// Creates a new owned message with metadata attached.
  #[must_use]
  pub fn with_metadata<T>(payload: T, metadata: MessageMetadata) -> Self
  where
    T: Any + Send + Sync + 'static, {
    Self::new(payload).with_metadata_owned(metadata)
  }

  /// Returns the dynamic type identifier of the payload.
  #[must_use]
  pub const fn type_id(&self) -> TypeId {
    self.type_id
  }

  /// Returns the reply target handle if present.
  #[must_use]
  pub const fn reply_to(&self) -> Option<&ActorRef> {
    self.reply_to.as_ref()
  }

  /// Sets the reply target handle.
  #[must_use]
  pub fn with_reply_to(mut self, reply_to: ActorRef) -> Self {
    self.reply_to = Some(reply_to);
    self
  }

  /// Attaches metadata to the message, replacing any existing entries.
  #[must_use]
  pub fn with_metadata_owned(mut self, metadata: MessageMetadata) -> Self {
    self.metadata = Some(metadata);
    self
  }

  /// Returns metadata if present.
  #[must_use]
  pub fn metadata(&self) -> Option<&MessageMetadata> {
    self.metadata.as_ref()
  }

  /// Borrows the message as an [`AnyMessage`].
  #[must_use]
  pub fn borrow(&self) -> AnyMessage<'_> {
    let payload = &*self.payload;
    AnyMessage::from_parts(payload, self.type_id, self.metadata.as_ref(), self.reply_to.as_ref())
  }

  /// Attempts to downcast the payload reference to the requested type.
  #[must_use]
  pub fn downcast_ref<T>(&self) -> Option<&T>
  where
    T: Any + 'static, {
    (&*self.payload).downcast_ref()
  }
}

impl fmt::Debug for AnyOwnedMessage {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("AnyOwnedMessage")
      .field("type_id", &self.type_id)
      .field("reply_to", &self.reply_to)
      .field("has_metadata", &self.metadata.is_some())
      .finish()
  }
}
