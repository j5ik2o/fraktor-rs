#[test]
fn remote_lifecycle_showcase_is_registered() {
  let manifest = include_str!("../../Cargo.toml");

  assert!(manifest.contains("fraktor-remote-core-rs"), "remote core dependency must be registered");
  assert!(manifest.contains("fraktor-remote-adaptor-std-rs"), "remote std adaptor dependency must be registered");
  assert!(manifest.contains("name = \"remote_lifecycle\""), "remote lifecycle example must be registered");
  assert!(
    manifest.contains("path = \"legacy/remote_lifecycle/main.rs\""),
    "remote lifecycle example path must be registered",
  );
}

#[test]
fn remote_lifecycle_showcase_uses_public_remote_lifecycle_api() {
  let source = include_str!("../remote_lifecycle/main.rs");

  assert!(source.contains("RemotingExtensionInstaller"), "showcase must install the remote extension");
  assert!(source.contains("TcpRemoteTransport"), "showcase must configure the std TCP transport");
  assert!(source.contains("RemoteConfig::new"), "showcase must pass remote config to the extension installer");
  assert!(source.contains("with_extension_installers"), "showcase must install remoting through ActorSystemConfig");
  assert!(
    source.contains("with_shared_extension_installer"),
    "showcase must pass the installer through ActorSystemConfig",
  );
  assert!(
    !source.contains(".install(&system)"),
    "showcase must not install remoting directly after ActorSystem bootstrap",
  );
  assert!(!source.contains(".remote()"), "showcase must not fetch the internal remote handle");
  assert!(!source.contains(".start()"), "showcase must not manually start remoting");
  assert!(!source.contains("spawn_run_task"), "showcase must not manually spawn the remote run task");
  assert!(!source.contains("shutdown_and_join"), "showcase must not manually join remoting shutdown");
}
