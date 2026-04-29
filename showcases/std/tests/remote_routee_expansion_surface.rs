#[test]
fn remote_routee_expansion_showcase_is_registered() {
  let manifest = include_str!("../Cargo.toml");

  assert!(
    manifest.contains("name = \"remote_routee_expansion\""),
    "remote routee expansion example must be registered"
  );
  assert!(
    manifest.contains("path = \"remote_routee_expansion/main.rs\""),
    "remote routee expansion example path must be registered",
  );
}

#[test]
fn remote_routee_expansion_showcase_uses_public_runtime_expansion_api() {
  let source = include_str!("../remote_routee_expansion/main.rs");

  assert!(source.contains("RemoteRouteeExpansion"), "showcase must use RemoteRouteeExpansion");
  assert!(source.contains("RemoteRouterConfig::new"), "showcase must configure RemoteRouterConfig");
  assert!(source.contains("StdRemoteActorRefProvider::new"), "showcase must resolve routees through std provider");
  assert!(source.contains("RoundRobinPool::new"), "showcase must demonstrate a supported remote pool variant");
  assert!(
    !source.contains("#[allow(clippy::print_stdout)]"),
    "remote routee expansion showcase must not suppress clippy::print_stdout",
  );
}
