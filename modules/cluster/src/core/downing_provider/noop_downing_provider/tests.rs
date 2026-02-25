use crate::core::downing_provider::{DowningProvider, NoopDowningProvider};

#[test]
fn noop_downing_provider_accepts_down_command() {
  let mut provider = NoopDowningProvider::new();
  assert!(provider.down("node-a:2552").is_ok());
}
