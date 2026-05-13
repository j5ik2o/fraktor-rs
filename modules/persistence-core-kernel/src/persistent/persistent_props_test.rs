use fraktor_actor_core_kernel_rs::actor::{
  ActorContext, error::ActorError, messaging::AnyMessageView, props::MailboxRequirement,
};

use crate::{
  persistent::{Eventsourced, PersistenceContext, PersistentActor, PersistentRepr, persistent_props},
  snapshot::Snapshot,
};

struct TestPersistentActor {
  context: PersistenceContext<Self>,
}

impl TestPersistentActor {
  fn new() -> Self {
    Self { context: PersistenceContext::new("persistent-props-test".into()) }
  }
}

impl Eventsourced for TestPersistentActor {
  fn persistence_id(&self) -> &str {
    self.context.persistence_id()
  }

  fn receive_recover(&mut self, _event: &PersistentRepr) {}

  fn receive_snapshot(&mut self, _snapshot: &Snapshot) {}

  fn receive_command(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn last_sequence_nr(&self) -> u64 {
    self.context.last_sequence_nr()
  }
}

impl PersistentActor for TestPersistentActor {
  fn persistence_context(&mut self) -> &mut PersistenceContext<Self> {
    &mut self.context
  }
}

#[test]
fn persistent_props_requires_stash_mailbox() {
  let props = persistent_props(TestPersistentActor::new);

  assert_eq!(props.mailbox_requirement(), MailboxRequirement::for_stash());
}
