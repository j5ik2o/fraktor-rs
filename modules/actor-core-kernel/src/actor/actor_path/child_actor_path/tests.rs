use super::{
  super::{ActorPath, ActorPathError, ActorPathParts, GuardianKind},
  ChildActorPath,
};

#[test]
fn try_from_path_accepts_child() {
  let path = ActorPath::root().child("worker");
  let child = ChildActorPath::try_from_path(path);
  assert!(child.is_ok());
  assert_eq!(child.unwrap().name(), "worker");
}

#[test]
fn try_from_path_rejects_root() {
  let path = ActorPath::root();
  let result = ChildActorPath::try_from_path(path);
  assert_eq!(result, Err(ActorPathError::NotChildPath));
}

#[test]
fn from_parent_creates_child() {
  let root = ActorPath::root();
  let child = ChildActorPath::from_parent(&root, "service").unwrap();
  assert_eq!(child.name(), "service");
  assert_eq!(child.segments().len(), 2); // "user" + "service"
}

#[test]
fn from_parent_rejects_invalid_segment() {
  let root = ActorPath::root();
  let result = ChildActorPath::from_parent(&root, "$reserved");
  assert!(result.is_err());
}

#[test]
fn try_child_creates_deeper_child() {
  let path = ActorPath::root().child("service");
  let child = ChildActorPath::try_from_path(path).unwrap();
  let deeper = child.try_child("worker").unwrap();
  assert_eq!(deeper.name(), "worker");
  assert_eq!(deeper.segments().len(), 3); // "user" + "service" + "worker"
}

#[test]
fn name_returns_last_segment() {
  let path = ActorPath::root().child("service").child("worker");
  let child = ChildActorPath::try_from_path(path).unwrap();
  assert_eq!(child.name(), "worker");
}

#[test]
fn into_inner_returns_original_path() {
  let original = ActorPath::root().child("worker");
  let child = ChildActorPath::try_from_path(original.clone()).unwrap();
  assert_eq!(child.into_inner(), original);
}

#[test]
fn display_matches_inner_path() {
  let path = ActorPath::root().child("worker");
  let child = ChildActorPath::try_from_path(path.clone()).unwrap();
  assert_eq!(child.to_string(), path.to_string());
}

#[test]
fn from_conversion_into_actor_path() {
  let path = ActorPath::root().child("worker");
  let child = ChildActorPath::try_from_path(path.clone()).unwrap();
  let converted: ActorPath = child.into();
  assert_eq!(converted, path);
}

#[test]
fn canonical_uri_delegates_to_inner() {
  let parts = ActorPathParts::local("testsys")
    .with_guardian(GuardianKind::User)
    .with_authority_host("host.example.com".into())
    .with_authority_port(2552);
  let path = ActorPath::from_parts(parts).child("worker");
  let child = ChildActorPath::try_from_path(path.clone()).unwrap();
  assert_eq!(child.to_canonical_uri(), path.to_canonical_uri());
}

#[test]
fn as_path_and_relative_string_delegate_to_inner() {
  let path = ActorPath::root().child("service").child("worker");
  let child = ChildActorPath::try_from_path(path.clone()).unwrap();

  assert_eq!(child.as_path(), &path);
  assert_eq!(child.to_relative_string(), "/user/service/worker");
}

#[test]
fn debug_uses_child_relative_path() {
  let path = ActorPath::root().child("worker");
  let child = ChildActorPath::try_from_path(path).unwrap();

  assert_eq!(alloc::format!("{child:?}"), "ChildActorPath(\"/user/worker\")");
}

#[test]
fn clone_produces_equal_child() {
  let path = ActorPath::root().child("worker");
  let child = ChildActorPath::try_from_path(path).unwrap();
  let cloned = child.clone();
  assert_eq!(child, cloned);
}

#[test]
fn system_guardian_child_path() {
  let path = ActorPath::root_with_guardian(GuardianKind::System).child("logger");
  let child = ChildActorPath::try_from_path(path).unwrap();
  assert_eq!(child.name(), "logger");
  assert_eq!(child.parts().guardian(), GuardianKind::System);
}

#[test]
fn try_from_trait_accepts_child() {
  let path = ActorPath::root().child("worker");
  let child = ChildActorPath::try_from(path);
  assert!(child.is_ok());
  assert_eq!(child.unwrap().name(), "worker");
}

#[test]
fn try_from_trait_rejects_root() {
  let path = ActorPath::root();
  let result = ChildActorPath::try_from(path);
  assert_eq!(result, Err(ActorPathError::NotChildPath));
}
