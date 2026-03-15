extern crate alloc;
extern crate std;

use alloc::string::String;

use fraktor_utils_rs::core::sync::ArcShared;

use crate::{
  core::dispatch::mailbox::{Mailbox, MailboxPolicy},
  std::dispatch::dispatcher::PinnedDispatcher,
};

#[test]
fn default_prefix_is_fraktor_pinned() {
  let pd = PinnedDispatcher::new();
  assert_eq!(pd.thread_name_prefix(), "fraktor-pinned");
}

#[test]
fn custom_prefix_is_preserved() {
  let pd = PinnedDispatcher::with_thread_name_prefix("my-actor");
  assert_eq!(pd.thread_name_prefix(), "my-actor");
}

#[test]
fn default_trait_matches_new() {
  let a = PinnedDispatcher::new();
  let b = PinnedDispatcher::default();
  assert_eq!(a.thread_name_prefix(), b.thread_name_prefix());
}

#[test]
fn build_config_returns_blocking_capable_executor() {
  let pd = PinnedDispatcher::new();
  let config = pd.build_config();
  assert!(config.executor().supports_blocking());
}

#[test]
fn each_build_config_creates_independent_executor() {
  let pd = PinnedDispatcher::new();
  let config_a = pd.build_config();
  let config_b = pd.build_config();

  // Two configs must use distinct executor runners (not the same ArcShared).
  assert!(!ArcShared::ptr_eq(&config_a.executor(), &config_b.executor()));
}

#[test]
fn build_config_produces_valid_dispatcher() {
  let pd = PinnedDispatcher::new();
  let config = pd.build_config();
  let mailbox = ArcShared::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  let result = config.build_dispatcher(mailbox);
  assert!(result.is_ok());
}

#[test]
fn string_into_prefix() {
  let owned: String = String::from("owned-prefix");
  let pd = PinnedDispatcher::with_thread_name_prefix(owned);
  assert_eq!(pd.thread_name_prefix(), "owned-prefix");
}
