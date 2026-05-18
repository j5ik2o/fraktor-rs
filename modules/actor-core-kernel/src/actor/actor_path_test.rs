use alloc::{vec, vec::Vec};

use super::{
  ActorPath, ActorPathComparator, ActorPathError, ActorPathFormatter, ActorPathParser, ActorPathParts, ActorUid,
  GuardianKind, PathResolutionError, PathSegment,
};

#[test]
fn guardian_segment_is_injected_into_root() {
  let parts = ActorPathParts::local("cellsys").with_guardian(GuardianKind::System);
  let path = ActorPath::from_parts(parts);
  let segment_names: Vec<&str> = path.segments().iter().map(PathSegment::as_str).collect();
  assert_eq!(segment_names, vec!["system"]);
}

#[test]
fn root_injects_user_guardian_by_default() {
  let path = ActorPath::root();
  let segment_names: Vec<&str> = path.segments().iter().map(PathSegment::as_str).collect();
  assert_eq!(segment_names, vec!["user"]);
}

#[test]
fn path_segment_rejects_reserved_dollar_prefix() {
  let result = PathSegment::new("$user");
  assert!(result.is_err());
}

#[test]
fn path_segment_rejects_empty_invalid_percent_and_invalid_char() {
  assert!(matches!(PathSegment::new(""), Err(ActorPathError::EmptySegment)));
  assert!(matches!(PathSegment::new("bad%"), Err(ActorPathError::InvalidPercentEncoding)));
  assert!(matches!(PathSegment::new("bad%XX"), Err(ActorPathError::InvalidPercentEncoding)));
  assert!(matches!(PathSegment::new("bad space"), Err(ActorPathError::InvalidSegmentChar { ch: ' ', index: 3 })));
}

#[test]
fn actor_path_parts_can_set_port_before_host() {
  let parts = ActorPathParts::local("cellsys").with_authority_port(2552);

  assert_eq!(parts.authority_endpoint().as_deref(), Some(":2552"));
}

#[test]
fn canonical_uri_includes_scheme_system_and_segments() {
  let parts = ActorPathParts::local("cellsys")
    .with_guardian(GuardianKind::User)
    .with_authority_host("host.example.com".into())
    .with_authority_port(2552);
  let path = ActorPath::from_parts(parts).child("service").child("worker");
  let canonical = ActorPathFormatter::format(&path);
  assert_eq!(canonical, "fraktor://cellsys@host.example.com:2552/user/service/worker");
}

#[test]
fn comparator_ignores_uid_difference() {
  let base = ActorPath::root().child("worker");
  let with_uid = base.clone().with_uid(ActorUid::new(42));
  assert!(ActorPathComparator::eq(&base, &with_uid));
  assert_eq!(ActorPathComparator::hash(&base), ActorPathComparator::hash(&with_uid));
}

#[test]
fn parser_rejects_reserved_segment() {
  let uri = "fraktor://sys/user/$system";
  assert!(ActorPathParser::parse(uri).is_err());
}

#[test]
fn parser_preserves_authority_and_guardian() {
  let uri = "fraktor.tcp://cellsys@host.example.com:2552/system/logger";
  let parsed = ActorPathParser::parse(uri).expect("parse");
  assert_eq!(parsed.parts().system(), "cellsys");
  assert_eq!(parsed.parts().guardian_segment(), "system");
  assert_eq!(parsed.segments().iter().map(PathSegment::as_str).collect::<Vec<_>>(), vec!["system", "logger"]);
  assert_eq!(ActorPathFormatter::format(&parsed), uri);
}

#[test]
fn format_parse_roundtrip_samples() {
  let cases = [
    "fraktor://cellsys/user/worker",
    "fraktor.tcp://cellsys@127.0.0.1:2552/user/service#7",
    "fraktor://cellsys/system/logger/sub",
  ];
  for uri in cases {
    let parsed = ActorPathParser::parse(uri).expect("parse");
    let formatted = ActorPathFormatter::format(&parsed);
    assert_eq!(formatted, uri);
  }
}

#[test]
fn parser_decodes_percent_encoded_segments() {
  let uri = "fraktor://cellsys/user/service%20worker";
  let parsed = ActorPathParser::parse(uri).expect("parse");
  let last = parsed.segments().last().expect("segment");
  assert_eq!(last.as_str(), "service%20worker");
  assert_eq!(last.decoded(), "service worker");
}

#[test]
fn actor_path_error_display_matches_public_contract() {
  // Pekko contract: actor path construction errors must be observable and
  // diagnostic without exposing parser internals to callers.
  let cases = [
    (ActorPathError::EmptySegment, "path segment must not be empty"),
    (ActorPathError::ReservedSegment, "path segment must not start with '$'"),
    (ActorPathError::InvalidSegmentChar { ch: ' ', index: 3 }, "invalid character ' ' at position 3"),
    (ActorPathError::InvalidPercentEncoding, "invalid percent encoding sequence"),
    (ActorPathError::RelativeEscape, "relative path escapes beyond guardian root"),
    (ActorPathError::InvalidUri, "invalid actor path uri"),
    (ActorPathError::UnsupportedScheme, "unsupported actor path scheme"),
    (ActorPathError::MissingSystemName, "missing actor system name"),
    (ActorPathError::InvalidAuthority, "invalid authority segment"),
    (ActorPathError::NotRootPath, "path is not a root path"),
    (ActorPathError::NotChildPath, "path is not a child path"),
  ];

  for (error, expected) in cases {
    assert_eq!(error.to_string(), expected);
  }
}

#[test]
fn path_resolution_error_display_matches_public_contract() {
  // Registry / remote path resolution errors are part of the public failure
  // surface even when the concrete registry implementation changes.
  let cases = [
    (PathResolutionError::PidUnknown, "PID not found in registry"),
    (PathResolutionError::AuthorityUnresolved, "authority is not resolved"),
    (PathResolutionError::AuthorityQuarantined, "authority is quarantined"),
    (PathResolutionError::UidReserved { uid: ActorUid::new(42) }, "UID 42 is reserved and cannot be reused"),
  ];

  for (error, expected) in cases {
    assert_eq!(error.to_string(), expected);
  }
}
