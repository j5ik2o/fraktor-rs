use super::{
  super::{ActorPath, ActorPathError, ActorPathParts, GuardianKind},
  RootActorPath,
};

#[test]
fn new_creates_user_guardian_root() {
  let root = RootActorPath::new();
  assert_eq!(root.guardian(), GuardianKind::User);
  assert_eq!(root.as_path().segments().len(), 1);
  assert_eq!(root.as_path().segments()[0].as_str(), "user");
}

#[test]
fn with_guardian_creates_system_root() {
  let root = RootActorPath::with_guardian(GuardianKind::System);
  assert_eq!(root.guardian(), GuardianKind::System);
  assert_eq!(root.as_path().segments()[0].as_str(), "system");
}

#[test]
fn from_parts_preserves_metadata() {
  let parts = ActorPathParts::local("testsys")
    .with_guardian(GuardianKind::System)
    .with_authority_host("host.example.com".into())
    .with_authority_port(2552);
  let root = RootActorPath::from_parts(parts);
  assert_eq!(root.parts().system(), "testsys");
  assert_eq!(root.guardian(), GuardianKind::System);
}

#[test]
fn try_from_path_accepts_root() {
  let path = ActorPath::root();
  let root = RootActorPath::try_from_path(path);
  assert!(root.is_ok());
}

#[test]
fn try_from_path_rejects_child() {
  let path = ActorPath::root().child("worker");
  let result = RootActorPath::try_from_path(path);
  assert_eq!(result, Err(ActorPathError::NotRootPath));
}

#[test]
fn into_inner_returns_original_path() {
  let root = RootActorPath::new();
  let original = ActorPath::root();
  assert_eq!(root.into_inner(), original);
}

#[test]
fn display_matches_inner_path() {
  let root = RootActorPath::new();
  let inner = ActorPath::root();
  assert_eq!(root.to_string(), inner.to_string());
}

#[test]
fn from_conversion_into_actor_path() {
  let root = RootActorPath::new();
  let path: ActorPath = root.into();
  assert_eq!(path, ActorPath::root());
}

#[test]
fn default_creates_user_guardian_root() {
  let root = RootActorPath::default();
  assert_eq!(root.guardian(), GuardianKind::User);
}

#[test]
fn clone_produces_equal_root() {
  let root = RootActorPath::with_guardian(GuardianKind::System);
  let cloned = root.clone();
  assert_eq!(root, cloned);
}
