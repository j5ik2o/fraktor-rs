use super::ActorRefResolveError;

#[test]
fn display_matches_public_contract() {
  let cases = [
    (ActorRefResolveError::UnsupportedScheme, "unsupported actor path scheme"),
    (ActorRefResolveError::ProviderMissing, "no actor-ref provider registered for scheme"),
    (ActorRefResolveError::SystemNotBootstrapped, "actor system not bootstrapped yet"),
    (ActorRefResolveError::InvalidAuthority, "authority is missing or incomplete"),
    (ActorRefResolveError::NotFound("missing actor".into()), "actor reference could not be resolved: missing actor"),
  ];

  for (error, expected) in cases {
    assert_eq!(alloc::format!("{error}"), expected);
  }
}
