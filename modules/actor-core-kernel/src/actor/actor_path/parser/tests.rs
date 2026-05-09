use alloc::{vec, vec::Vec};

use fraktor_utils_core_rs::net::UriError;

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
fn parse_remote_ipv6_authority_without_port() {
  let uri = "fraktor://remote-sys@[::1]/user/worker";
  let path = ActorPathParser::parse(uri).expect("parse ipv6");

  assert_eq!(path.parts().system(), "remote-sys");
  assert_eq!(path.parts().authority_endpoint().as_deref(), Some("[::1]"));
  assert_eq!(path.segments().iter().map(PathSegment::as_str).collect::<Vec<_>>(), vec!["user", "worker"]);
}

#[test]
fn parse_remote_host_without_port() {
  let uri = "fraktor://remote-sys@host.example.com/system/logger";
  let path = ActorPathParser::parse(uri).expect("parse host without port");

  assert_eq!(path.parts().system(), "remote-sys");
  assert_eq!(path.parts().authority_endpoint().as_deref(), Some("host.example.com"));
  assert_eq!(path.parts().guardian_segment(), "system");
}

#[test]
fn parse_empty_path_returns_system_root_without_segments() {
  let path = ActorPathParser::parse("fraktor://empty-sys").expect("parse root");

  assert_eq!(path.parts().system(), "empty-sys");
  assert_eq!(path.to_relative_string(), "/user");
}

#[test]
fn reject_unknown_scheme() {
  let err = ActorPathParser::parse("unknown://sys/user/a").unwrap_err();
  assert!(matches!(err, ActorPathError::UnsupportedScheme));
}

#[test]
fn parse_accepts_uri_without_explicit_scheme() {
  let path = ActorPathParser::parse("//sys/user/a").expect("parse");

  assert_eq!(path.parts().system(), "sys");
  assert_eq!(path.segments().iter().map(PathSegment::as_str).collect::<Vec<_>>(), vec!["user", "a"]);
}

#[test]
fn reject_empty_system_name() {
  let err = ActorPathParser::parse("fraktor://@host.example.com/user/a").unwrap_err();

  assert!(matches!(err, ActorPathError::MissingSystemName));
}

#[test]
fn reject_empty_authority_host() {
  let err = ActorPathParser::parse("fraktor://sys@/user/a").unwrap_err();

  assert!(matches!(err, ActorPathError::InvalidAuthority));
}

#[test]
fn reject_empty_host_before_port() {
  let err = ActorPathParser::parse("fraktor://sys@:2552/user/a").unwrap_err();

  assert!(matches!(err, ActorPathError::InvalidAuthority));
}

#[test]
fn reject_invalid_port() {
  let err = ActorPathParser::parse("fraktor://sys@host.example.com:notaport/user/a").unwrap_err();

  assert!(matches!(err, ActorPathError::InvalidAuthority));
}

#[test]
fn reject_invalid_ipv6_authority() {
  let err = ActorPathParser::parse("fraktor://sys@[::1/user/a").unwrap_err();

  assert!(matches!(err, ActorPathError::InvalidAuthority));
}

#[test]
fn uri_error_conversion_maps_to_invalid_uri() {
  assert!(matches!(ActorPathError::from(UriError::InvalidPath), ActorPathError::InvalidUri));
}
