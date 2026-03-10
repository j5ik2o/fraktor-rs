#[test]
fn std_pattern_reexports_core_helpers() {
  let _ = crate::std::pattern::ask_with_timeout;
  let _ = crate::std::pattern::graceful_stop;
  let _ = crate::std::pattern::graceful_stop_with_message;
  let mut delay_provider = fraktor_utils_rs::core::timing::delay::ManualDelayProvider::new();
  let _future = crate::std::pattern::retry(
    1,
    &mut delay_provider,
    |_| core::time::Duration::ZERO,
    || core::future::ready(Ok::<(), ()>(())),
  );
}
