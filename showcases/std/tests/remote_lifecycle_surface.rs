#[test]
fn remote_lifecycle_showcase_is_registered() {
  let manifest = include_str!("../Cargo.toml");

  assert!(manifest.contains("fraktor-remote-core-rs"), "remote core dependency must be registered");
  assert!(manifest.contains("fraktor-remote-adaptor-std-rs"), "remote std adaptor dependency must be registered");
  assert!(manifest.contains("name = \"remote_lifecycle\""), "remote lifecycle example must be registered");
  assert!(manifest.contains("path = \"remote_lifecycle/main.rs\""), "remote lifecycle example path must be registered");
}

#[test]
fn remote_lifecycle_showcase_uses_public_remote_lifecycle_api() {
  let source = include_str!("../remote_lifecycle/main.rs");

  assert!(source.contains("RemotingExtensionInstaller"), "showcase must install the remote extension");
  assert!(source.contains("TcpRemoteTransport"), "showcase must configure the std TCP transport");
  assert!(source.contains("RemotingLifecycleEvent::ListenStarted"), "showcase must observe listen lifecycle events");
  assert!(!source.contains("#[allow(clippy::print_stdout)]"), "showcase must not suppress print stdout lint");
}
