use alloc::{boxed::Box, string::String};

use crate::actor::{
  Actor, ActorContext,
  error::ActorError,
  messaging::{AnyMessage, AnyMessageView},
  props::{DeployableActorFactoryRegistry, DeployableFactoryError, DeployableFactoryLookupError, Props},
};

struct TestActor;

impl Actor for TestActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _msg: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn registry_resolves_factory_by_stable_id() {
  let mut registry = DeployableActorFactoryRegistry::new();
  registry.register(
    "echo",
    Box::new(|payload: AnyMessage| {
      assert_eq!(payload.downcast_ref::<String>().map(String::as_str), Some("payload"));
      Ok(Props::from_fn(|| TestActor))
    }),
  );

  let props = registry
    .props_for_payload("echo", AnyMessage::new(String::from("payload")))
    .expect("registered factory should build props");

  assert!(props.factory().is_some());
}

#[test]
fn registry_rejects_unknown_factory_id() {
  let registry = DeployableActorFactoryRegistry::new();

  let error = match registry.props_for_payload("missing", AnyMessage::new(())) {
    | Ok(_) => panic!("unknown factory id should fail"),
    | Err(error) => error,
  };

  assert_eq!(error, DeployableFactoryLookupError::UnknownFactoryId(String::from("missing")));
}

#[test]
fn registry_surfaces_factory_rejection() {
  let mut registry = DeployableActorFactoryRegistry::new();
  registry.register("echo", Box::new(|_payload: AnyMessage| Err(DeployableFactoryError::new("bad payload"))));

  let error = match registry.props_for_payload("echo", AnyMessage::new(())) {
    | Ok(_) => panic!("factory rejection should fail"),
    | Err(error) => error,
  };

  assert_eq!(error, DeployableFactoryLookupError::FactoryRejected(DeployableFactoryError::new("bad payload")));
}
