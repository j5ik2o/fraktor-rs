extern crate alloc;
extern crate std;

use alloc::string::String;

use fraktor_actor_core_rs::core::kernel::dispatch::{
  dispatcher::{DispatcherProvider, DispatcherProvisionRequest, DispatcherSettings},
  mailbox::{Mailbox, MailboxPolicy},
};
use fraktor_utils_rs::core::sync::ArcShared;

use crate::std::dispatch::dispatcher::PinnedDispatcher;

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
fn provision_returns_dispatcher_that_builds_runtime_dispatcher() {
  let pd = PinnedDispatcher::new();
  let request = DispatcherProvisionRequest::new("pinned").with_actor_name("worker");
  let dispatcher = pd.provision(&DispatcherSettings::default(), &request).expect("provision");
  let mailbox = ArcShared::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  let result = dispatcher.build_dispatcher(mailbox);
  assert!(result.is_ok());
}

#[test]
fn each_provision_creates_independent_dispatchers() {
  let pd = PinnedDispatcher::new();
  let settings = DispatcherSettings::default();
  let a = pd
    .provision(&settings, &DispatcherProvisionRequest::new("pinned").with_actor_name("worker-a"))
    .expect("first provision");
  let b = pd
    .provision(&settings, &DispatcherProvisionRequest::new("pinned").with_actor_name("worker-b"))
    .expect("second provision");
  let mailbox_a = ArcShared::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  let mailbox_b = ArcShared::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  let dispatcher_a = a.build_dispatcher(mailbox_a).expect("first dispatcher");
  let dispatcher_b = b.build_dispatcher(mailbox_b).expect("second dispatcher");

  let _ = (dispatcher_a, dispatcher_b);
}

#[test]
fn string_into_prefix() {
  let owned: String = String::from("owned-prefix");
  let pd = PinnedDispatcher::with_thread_name_prefix(owned);
  assert_eq!(pd.thread_name_prefix(), "owned-prefix");
}
