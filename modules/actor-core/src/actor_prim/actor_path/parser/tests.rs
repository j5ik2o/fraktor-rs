use alloc::{vec, vec::Vec};

use super::*;

#[test]
fn parse_local_user_path() {
  let path = ActorPathParser::parse("fraktor://my-sys/user/service/worker").expect("parse");
  assert_eq!(path.parts().system(), "my-sys");
  assert_eq!(path.parts().guardian_segment(), "user");
  assert_eq!(path.segments().iter().map(PathSegment::as_str).collect::<Vec<_>>(), vec!["user", "service", "worker"]);
  assert!(path.uid().is_none());
}

#[test]
fn parse_remote_with_uid() {
  let uri = "fraktor.tcp://remote-sys@host.example.com:2552/system/logger#42";
  let path = ActorPathParser::parse(uri).expect("parse");
  assert_eq!(path.parts().system(), "remote-sys");
  assert_eq!(path.parts().guardian_segment(), "system");
  assert_eq!(path.parts().scheme(), ActorPathScheme::FraktorTcp);
  assert_eq!(path.uid().map(|uid| uid.value()), Some(42));
  assert_eq!(path.segments().iter().map(PathSegment::as_str).collect::<Vec<_>>(), vec!["system", "logger"]);
}

#[test]
fn reject_unknown_scheme() {
  let err = ActorPathParser::parse("unknown://sys/user/a").unwrap_err();
  assert!(matches!(err, ActorPathError::UnsupportedScheme));
}
